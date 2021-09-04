# Hoard

## TODO
### Project Ideas
* Execute command like `homemaker`?
* Work with the encryption
* Add all options to global configuration as well
* Run threads on the receiver end of `WalkBuilder`
* Option to add things to configuration from the command line
* Allow default configuration to be many formats, not just `config.toml`

### Working and Compatibility
* Ensure more tests
* Confirm this works `[[]]` in `yaml`

## Fork
* Convert configuration file type from `json`, `yaml`, and `toml`
  * As of now, this new configuration that is not located in hoards default directory has to be `config.toml`
    * When using `-c|--config` option, any format can be read
  * Uses `-C|--color` for colored output when printing to `stdout`
  * Uses same exact methods that `bat` does. Can specify theme with `-t|--theme`
```sh
# Writes to stdout
hoard -c config.toml config -xf json
# Writes to file
hoard -c config.json config -xf yaml -o new.yaml
# Writes to stdout with colored output
hoard -c config.yaml config -xf toml -Ct <theme>
```
* Has a colored help message
* Expands both `~` and environment variables in `path_exists` as well as the `hoard`'s file path
  * Has ability to parse default variable settings that use another variable (i.e., `${ZDOTDIR:-$HOME/.config/zsh}`)
  * Variables do not need to be surrounded by curly braces unless the default value is given (i.e., `$ZDOTDIR`)
* `macOS` configuration directories now respect the `XDG` data structure
* Ability to use three different file formats: `toml` or `yaml`/`yml`, and `json`
  * Depending on the extension of the file that is given using the `-c|--config` parameter will determine which is parsed
  * If there is no extension on the file then `toml` will be used
  * Some people may find one or the other easier to read
* Uses the crate `ignore` when walking directories
  * This provides **many** more options that can be given to the user when building directory
  * Note that not all of the fields need to be filled out, this is just what is available
  * Checkout `sample` directory for some configurations
  * These can be used like the following:
```toml
[global_config]
  "ignores" = [".git/"] # Array of gitignore like patterns
[hoards]
[hoards.file]
  [hoards.file.config]
    "follow_links"   = true
    "hidden"         = true
    "max_depth"      = 3
    "exclude"        = ["*.git*", "*another*"] # This is a pattern like regex, not an ignore pattern
    "pattern"        = "*.txt"
    "regex"          = false
    "case_sensitive" = false
    [hoards.file.config.encryption] # Work in progress
      "encrypt"      = "symmetric"
      "encrypt_pass" = "pass"
  [hoards.file.named]
    "env|unix" = "$HOME/test/file"
  [hoards.file.another]
    "env|unix" = "${HOME:-/Users/user}/test/file"
  [hoards.file.evenmore]
    "env|unix" = "${HOME:-$ZDOTDIR}/test/file"
```

This is where a `yaml` could be more legible and less verbose
```yaml
global_config:
  ignores: [".git/"] # Array of gitignore like patterns
hoards:
  file:
    config:
      follow_links: true
      hidden: true
      max_depth: 3
      exclude: ["*.git*", "*another*"]
      pattern: "*.txt"
      regex: false
      case_sensitive: false
      encryption: # Being worked on
        encrypt: symmetric
        encrypt_pass: lmao
    named:
      env|unix: $HOME/test/file
    another:
      env|unix: ${HOME:-$ZDOTDIR}/test/file
```

`hoard` is a program for backing up files from across a filesystem into a single directory
and restoring them later.

Most people will know these programs as "dotfile managers," where dotfiles are configuration
files on *nix (read: non-Windows) systems. Files on *nix systems are marked as hidden by
starting the file name with a dot (`.`).

## Terminology

- "Environment": An identifiable system configuration consisting of zero or more each of:
  operating system, hostname, environment variable, executables in `$PATH`, and/or existing
  paths,
- "Pile": A single file or directory with multiple possible paths where it can be found
  depending on the environment(s).
- "Hoard": One of:
  - A single anonymous pile.
  - One or more named, related piles.

## Usage

### Environment Variables
- `HOARD_CACHE_DIR`: directory where theme cache is stored (default: `${XDG_CACHE_HOME:-$HOME/.cache}/hoard`)
- `HOARD_DATA_DIR`: directory where hoard history is stored (default: `${XDG_DATA_HOME:-$HOME/.local/share}/hoard`)
- `HOARD_CONFIG_DIR`: directory where theme/hoard backups are stored (default: `${XDG_CONFIG_HOME:-$HOME/.config}/hoard`)

### Subcommands

- **Backup**: `hoard [flags...] backup [name] [name] [...]`
  - Back up the specified hoard(s). If no `name` is specified, all hoards are backed up.
- **Restore**: `hoard [flags...] restore [name] [name] [...]`
  - Restore the specified hoard(s). If no `name` is specified, all hoards are restored.
- **Validate**: `hoard [flags...] validate`
  - Attempt to parse the default configuration file (or the one provided via `--config-file`)
    Exits with code `0` if the config is valid.
- **Config**: `hoard [flags...] config [flags...]`
  - Modify configuration file by changing format
  - Display configuration file
- **Add**: `hoard [flags...] add`
  - Add items to configuration from command line

### Flags for `hoard`

- `--help`: View the program's help message.
- `-V/--version`: Print the version of `hoard`.
- `-c/--config-file`: Path to (non-default) configuration file.
- `-h/--hoards-root`: Path to (non-default) hoards root directory.

### Flags for `hoard config`

- `-x/--convert`: Convert file format
- `-i/--input-format`: Not really a necessary flag (reads from `-c/--config` option)
- `-f/--output-format`: Format to output when converting
- `-o/--output-file`: Format to output when converting
- `-C/--color`: Colorize output when printing to `stdout`
- `-t/--theme`: Theme name to use when colorizing output (found in `HOARD_`)
- `-B/--cache-build`: Build cache directory from (`$HOME/.config/hoard/themes`)
- `-R/--cache-clear`: Clear cache directory (`$HOME/.cache/hoard`)
- `-s/--source`: Source path to build cache directory
- `-d/--destination`: Destination path to build cache directory

### Verbosity

Output verbosity is controlled by the logging level. You can set the logging level with the
`HOARD_LOG` environment variable. Valid values (in decreasing verbosity) are:

- `trace`
- `debug`
- `info`
- `warn`
- `error`

The default logging level is `info` for release builds and `debug` for debugging builds.

### Default file locations

- Configuration file
  - Linux: `$XDG_CONFIG_HOME/hoard/config.toml` or `/home/$USER/.config/hoard/config.toml`
  - macos: `$XDG_CONFIG_HOME/hoard/config.toml` or `/Users/$USER/.config/hoard/config.toml`
  - Windows: `C:\Users\$USER\AppData\Roaming\shadow53\hoard\config.toml`
- Hoards root
  - Linux: `$XDG_DATA_HOME/hoard/hoards` or `/home/$USER/.local/share/hoard/hoards`
  - macos: `$XDG_DATA_HOME/hoard/hoards` or `/Users/$USER/.local/share/hoard/hoards`
  - Windows: `C:\Users\$USER\AppData\Roaming\shadow53\hoard\data\hoards`

More specifically, `hoard` uses the [`directories`](https://docs.rs/directories) library,
placing the configuration file in the `config_dir` and the hoards root in the `data_dir`.

## Configuration

See [`config.toml.sample`](config.toml.sample) for a documented example configuration
file.

### Environments

Environments can be matched on one or more of five possible factors:

- `os`: [Operating System](https://doc.rust-lang.org/stable/std/env/consts/constant.OS.html)
- `env`: Environment variables
  - Can match on just existence or also a specific value.
- `hostname`: The system hostname.
- `exe_exists`: Whether an executable file exists in `$PATH`.
- `path_exists`: Whether something exists (one of) the given path(s).

All the above factors can be written using two-dimensional array syntax. That is,
`["foo", ["bar, "baz"]]` is interpreted as `(foo) OR (bar AND baz)`, in whatever way applies
to that given factor.

It is an error to include an `AND` condition for `os` or `hostname`, as a system can only have
one of each.

```toml
[envs]
[envs.example_env]
    # Matching something *nix-y
    os = ["linux", "freebsd"]
    # Either sed and sh, or bash, must exist
    exe_exists = ["bash", ["sh", "sed"]]
    # Require both $HOME to exist and $HOARD_EXAMPLE_ENV to equal YES.
    # Note the double square brackets that indicate AND instead of OR.
    env = [[
      { var = "HOME" },
      { var = "HOARD_EXAMPLE_ENV", expected = "YES" },
    ]]
```

### Exclusivity

The exclusivity lists indicate names of environments that are considered mutually exclusive to
each other -- that is, cannot appear in the same environment condition -- and the order indicates
which one(s) have precedence when matching environments.

See the [example config file](config.toml.sample) for a more thorough example.

```toml
exclusivity = [
    # Assuming all else the same, an environment condition string with "neovim" will take
    # precedence over one with "vim", which takes precedence over one with "emacs".
    ["neovim", "vim", "emacs"]
]
```

### Hoards

Hoards consist of one or more piles, where each pile is a mapping of *environment condition
strings* to paths on the filesystem.

An *environment condition string* is one or more environment names separated by pipes. The
system must match ALL environments in the string in order for the associated path to be
considered.

The following rules determine which path to use for a pile:

1. The condition string with the most environments wins.
2. If multiple conditions have the most environments, the exclusivity list is used to
   determine if one takes precedence.
3. If multiple conditions have the same precedence, an error is printed and `hoard` exits.
4. If no conditions match, the pile is skipped and a warning is printed.

Note: it is possible that one condition may take precedence over another despite them not
having mutually exclusive environments between them, if one condition contains an environment
that shows up in the `exclusivity` list.

```toml
[hoards]
# This hoard consists of a single anonymous pile
[hoards.simple_hoard]
    # This is "foo" and "bar" separated by a pipe character (`|`).
    # It will use this path if the system matches both environments "foo" and "bar".
    "foo|bar" = "/path/to/a/thing"
    # This path is considered if the system matches the environment "baz".
    # It will use this path if one of "foo" or "bar" doesn't match. Otherwise, "foo|bar"
    # takes precedence because it is a longer condition (more environments to match).
    "baz" = "/some/different/path"

[hoards.complex_hoard]
# This hoard consists of two named piles: "first" and "second".
[hoards.complex_hoard.first]
    "foo|bar" = "/some/path/first"
    "baz" = "/some/different/path/first"
[hoards.complex_hoard.second]
    "foo|bar" = "/some/path/second"
    "baz" = "/some/different/path/second"
```
