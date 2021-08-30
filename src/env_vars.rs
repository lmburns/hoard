//! Expand environment variables inside of a path.
//!
//! The only function exported from this module is [`expand_env_in_path`].

use directories::BaseDirs;
use std::{env, path::PathBuf};

/// Takes the input string, expands all environment variables, and returns the
/// expanded string as a [`PathBuf`].
///
/// # Example
///
/// ```
/// use hoard::env_vars::expand_env_in_path;
/// use std::path::PathBuf;
///
/// let template = "/some/${CUSTOM_VAR}/path";
/// std::env::set_var("CUSTOM_VAR", "foobar");
/// let path = expand_env_in_path(template).expect("failed to expand path");
/// assert_eq!(path, PathBuf::from("/some/foobar/path"));
/// ```
///
/// # Errors
///
/// - Any [`VarError`](env::VarError) from looking up the environment variable's
///   value.
#[allow(clippy::missing_panics_doc)]
#[allow(clippy::too_many_lines)]
pub fn expand_env_in_path(path: &str) -> Result<PathBuf, env::VarError> {
    let new_path = path.to_owned();
    let _span = tracing::debug_span!("expand_env_in_path", %path).entered();

    let new_path = if new_path.starts_with('~') {
        tracing::trace!("found tilde in path {}", new_path);
        // Unwrap is safe since it was just checked that it started with what is being
        // stripped
        let no_tilde_path = new_path.strip_prefix('~').unwrap();
        if no_tilde_path.starts_with('/') || no_tilde_path.is_empty() {
            if let Some(base) = BaseDirs::new() {
                format!("{}{}", base.home_dir().display(), no_tilde_path)
            } else {
                new_path
            }
        } else {
            new_path
        }
    } else {
        new_path
    };

    // Taken from the crate [`shellexpand`] and modified here for better
    // error handling pertaining to the current crate.
    if let Some(idx) = new_path.find('$') {
        fn find_dollar(s: &str) -> usize {
            s.find('$').unwrap_or(s.len())
        }

        fn is_valid_var_name_char(c: char) -> bool {
            c.is_alphanumeric() || c == '_'
        }

        fn context(s: &str) -> Result<Option<String>, env::VarError> {
            match env::var(s) {
                Ok(value) => Ok(Some(value)),
                Err(env::VarError::NotPresent) => Ok(Some("".into())),
                Err(e) => Err(e),
            }
        }

        let mut res = String::with_capacity(new_path.len());
        let mut new_path = new_path.as_str();
        let mut dollar_idx = idx;

        loop {
            res.push_str(&new_path[..dollar_idx]);

            new_path = &new_path[dollar_idx..];
            if new_path.is_empty() {
                break;
            }

            let next_char = new_path[1..].chars().next();

            if next_char == Some('{') {
                if let Some(closing_brace_idx) = new_path.find('}') {
                    let mut default_value = None;
                    // Find default value (i.e., ${XDG_CONFIG_HOME:-$HOME/.config})
                    let var_name_end_idx = match new_path[..closing_brace_idx].find(":-") {
                        Some(default_split_idx) if default_split_idx != 2 => {
                            default_value =
                                Some(&new_path[default_split_idx + 2..closing_brace_idx]);
                            default_split_idx
                        },
                        _ => closing_brace_idx,
                    };

                    let var_name = &new_path[2..var_name_end_idx];
                    tracing::trace!(var_name, "found environment variable {}", var_name,);
                    match context(var_name) {
                        // if we have the variable set to some value
                        Ok(Some(var_value)) => {
                            res.push_str(var_value.as_ref());
                            new_path = &new_path[closing_brace_idx + 1..];
                            dollar_idx = find_dollar(new_path);
                        },

                        // if the variable is set and empty or if it is unset
                        not_found_or_empty => {
                            let value = match (not_found_or_empty, default_value) {
                                // return an error if we don't have a default and the variable is
                                // unset
                                (Err(err), None) => {
                                    return Err(err);
                                },
                                // use the default value if set
                                (_, Some(default)) => default,
                                // leave the variable as it is if the environment is empty
                                (_, None) => &new_path[..=closing_brace_idx],
                            };

                            res.push_str(value);
                            new_path = &new_path[closing_brace_idx + 1..];
                            dollar_idx = find_dollar(new_path);
                        },
                    }
                } else {
                    res.push_str(&new_path[..2]);
                    new_path = &new_path[2..];
                    dollar_idx = find_dollar(new_path);
                }
            } else if next_char.map(is_valid_var_name_char) == Some(true) {
                let end_idx = 2 + new_path[2..]
                    .find(|c: char| !is_valid_var_name_char(c))
                    .unwrap_or(new_path.len() - 2);

                let var_name = &new_path[1..end_idx];

                match context(var_name) {
                    Ok(var_value) =>
                        if let Some(var_value) = var_value {
                            tracing::trace!(
                                var_name,
                                path = %new_path,
                                %var_value,
                                "expanding first instance of variable in path"
                            );
                            res.push_str(var_value.as_ref());
                            new_path = &new_path[end_idx..];
                            dollar_idx = find_dollar(new_path);
                        } else {
                            res.push_str(&new_path[..end_idx]);
                            new_path = &new_path[end_idx..];
                            dollar_idx = find_dollar(new_path);
                        },
                    Err(e) => {
                        return Err(e);
                    },
                }
            } else {
                res.push('$');
                new_path = if next_char == Some('$') {
                    &new_path[2..] // skip the next dollar for escaping
                } else {
                    &new_path[1..]
                };
                dollar_idx = find_dollar(new_path);
            };
        }
        Ok(PathBuf::from(res).components().collect())
    } else {
        Ok(PathBuf::from(new_path).components().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! test_env {
        (
            name:
            $name:ident,input:
            $input:literal,env:
            $var:literal,value:
            $value:literal,expected:
            $expected:expr,require_var:
            $require_var:literal
        ) => {
            #[test]
            #[serial_test::serial]
            fn $name() {
                if $require_var && !($input).contains(&format!("${{{}}}", $var)) {
                    panic!("input string {} doesn't contain variable {}", $input, $var);
                }

                std::env::set_var($var, $value);
                let expected: PathBuf = $expected;
                let result = expand_env_in_path($input).expect("failed to expand env in path");
                assert_eq!(result, expected);
            }
        };
        (
            name:
            $name:ident,input:
            $input:literal,env:
            $var:literal,value:
            $value:literal,expected:
            $expected:expr
        ) => {
            test_env! {
                name: $name,
                input: $input,
                env: $var,
                value: $value,
                expected: $expected,
                require_var: true
            }
        };
    }

    test_env! {
        name: var_at_start_shorter_than_value,
        input: "${TEST_HOME}/test/file",
        env: "TEST_HOME",
        value: "/home/testuser",
        expected: PathBuf::from("/home/testuser/test/file")
    }

    test_env! {
        name: var_in_middle_shorter_than_value,
        input: "/home/testuser/${TEST_PATH}/file",
        env: "TEST_PATH",
        value: "test/subdir/subberdir",
        expected: PathBuf::from("/home/testuser/test/subdir/subberdir/file")
    }

    test_env! {
        name: var_at_end_shorter_than_value,
        input: "/home/testuser/${TEST_PATH}",
        env: "TEST_PATH",
        value: "test/subdir/file",
        expected: PathBuf::from("/home/testuser/test/subdir/file")
    }

    // Same length == var name + ${}
    test_env! {
        name: var_at_start_same_length_as_value,
        input: "${TEST_HOME}/test/file",
        env: "TEST_HOME",
        value: "/home/tester",
        expected: PathBuf::from("/home/tester/test/file")
    }

    test_env! {
        name: var_in_middle_same_length_as_value,
        input: "/home/testuser/${TEST_PATH}/file",
        env: "TEST_PATH",
        value: "/test/folder",
        expected: PathBuf::from("/home/testuser/test/folder/file")
    }

    test_env! {
        name: var_at_end_same_length_as_value,
        input: "/home/testuser/${TEST_PATH}",
        env: "TEST_PATH",
        value: "testing/file",
        expected: PathBuf::from("/home/testuser/testing/file")
    }

    test_env! {
        name: var_at_start_longer_than_value,
        input: "${TEST_HOME}/test/file",
        env: "TEST_HOME",
        value: "/home/test",
        expected: PathBuf::from("/home/test/test/file")
    }

    test_env! {
        name: var_in_middle_longer_than_value,
        input: "/home/testuser/${TEST_PATH}/file",
        env: "TEST_PATH",
        value: "test/dir",
        expected: PathBuf::from("/home/testuser/test/dir/file")
    }

    test_env! {
        name: var_at_end_longer_than_value,
        input: "/home/testuser/${TEST_PATH}",
        env: "TEST_PATH",
        value: "a/file",
        expected: PathBuf::from("/home/testuser/a/file")
    }

    test_env! {
        name: path_without_var_stays_same,
        input: "/path/without/variables",
        env: "UNUSED",
        value: "NOTHING",
        expected: PathBuf::from("/path/without/variables"),
        require_var: false
    }

    test_env! {
        name: path_with_two_variables,
        input: "/home/${TEST_USER}/somedir/${TEST_USER}/file",
        env: "TEST_USER",
        value: "testuser",
        expected: PathBuf::from("/home/testuser/somedir/testuser/file")
    }

    test_env! {
        name: var_without_braces_not_expanded,
        input: "/path/with/$INVALID/variable",
        env: "INVALID",
        value: "broken",
        expected: PathBuf::from("/path/with/$INVALID/variable"),
        require_var: false
    }

    test_env! {
        name: var_windows_style_not_expanded,
        input: "/path/with/%INVALID%/variable",
        env: "INVALID",
        value: "broken",
        expected: PathBuf::from("/path/with/%INVALID%/variable"),
        require_var: false
    }

    test_env! {
        name: vars_not_recursively_expanded,
        input: "${TEST_HOME}",
        env: "TEST_HOME",
        value: "${HOME}",
        expected: PathBuf::from("${HOME}")
    }

    test_env! {
        name: var_inside_var,
        input: "${WRAPPING${TEST_VAR}VARIABLE}",
        env: "TEST_VAR",
        value: "_",
        expected: PathBuf::from("${WRAPPING_VARIABLE}")
    }
}
