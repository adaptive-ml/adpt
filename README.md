# adpt

A command line tool for interacting with the Adaptive Platform.

## Installation

### MacOS

On ARM-based macs brew can be used:

```sh
brew install adaptive-ml/homebrew-tap/adpt
```

### Windows

On x86-based Windows winget can be used:

```powershell
winget install --source winget AdaptiveML.adpt
```

### Everything else

```sh
cargo install adpt
```

Once installed an API key must be specified for use. This can be done using the
`ADAPTIVE_API_KEY` environment variable, or alternatively stored in your
operating system's keyring using the below command:

```sh
adpt set-api-key
```

Additionally your adaptive instance may be specified either via the
`ADAPTIVE_BASE_URL` environment variable or via a configuration file as
described in the configuration section below.

### Completions

To set up completions for zsh run the following:

```sh
echo -e "\nsource <(COMPLETE=zsh adpt)" >> ~/.zshrc
```

Note that completions for things like recipe keys will only work when a default
project is configured.

## Usage

### Specifying the project

Most commands require a `--project` option to specify the project:

```sh
adpt recipes --project my-project
```

However to avoid specifying this every time, the `DEFAULT_PROJECT` environment
variable or the `default_project` configuration file option.:

### Setting API Key

Store your API key in the system keyring:

```sh
adpt set-api-key <your-api-key>
```

### Full command reference

For a complete list of commands see [[command-line-help-for-adpt]].

## Combining with other tools

In order to allow for easy scriptability adpt produces simple machine readable
output such as bare IDs when it is run in a pipe.

Below are a few examples of how adpt can be combined with other command line
utilities to achieve additional functionality.

### Publishing a recipe on save

You can use a tool such as [watchexec](https://github.com/watchexec/watchexec)
to run the publish command when files change:

```sh
watchexec adpt publish my_recipe.py --force
```

### Running a recipe on publish

The built-in `xargs` command can be used to use the output of one command as a
parameter to another:

```sh
adpt publish my_recipe.py | xargs -I {} adpt run {}
```

## Configuration

### Env file

Environment variables may be specified using a `.env` file in a parent folder.

### Configuration File Locations

Configuration files are stored in platform-specific locations:

| Platform    | Configuration File Path                                             |
| ----------- | ------------------------------------------------------------------- |
| **Linux**   | `~/.config/adpt/config.toml` or `$XDG_CONFIG_HOME/adpt/config.toml` |
| **macOS**   | `~/.adpt/config.toml`                                               |
| **Windows** | `%APPDATA%\adaptive-ml\adpt\config\config.toml`                     |

### Configuration File Format

The configuration file uses TOML format and supports the following options:

```toml
# Default project for operations
default_project = "my-project"

# Base URL for the Adaptive platform
adaptive_base_url = "https://your-adaptive-instance.com"
```

### API Key Storage

The API key can be provided in two ways (in order of priority):

1. **Environment Variable**: Set `ADAPTIVE_API_KEY` environment variable
2. **System Keyring**: Store securely using `adpt set-api-key <your-key>`
