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

### Fedora

A RPM is attached to the latest release on GitHub, simply download it and
install via DNF.

### Everything else

```sh
cargo install adpt
```

Once installed, configure a deployment (URL + API key + optional default
project) interactively:

```sh
adpt deployment setup prod
```

You can configure multiple deployments and switch between them with
`adpt deployment use <name>`. See the [Deployments](#deployments) section
below.

Environment variables (`ADAPTIVE_API_KEY`, `ADAPTIVE_BASE_URL`,
`DEFAULT_PROJECT`) still work and override the active deployment's values
on a per-invocation basis.

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

## Deployments

A deployment bundles a base URL, an API key (stored in the OS keyring), and
an optional default project. You can configure several and switch between
them.

```sh
adpt deployment setup prod         # interactive create/edit
adpt deployment setup staging
adpt deployment list               # see them all
adpt deployment show prod          # see the details of one
adpt deployment use staging        # pin this shell to staging
adpt deployment use prod --persist # pin this shell AND make prod the default for new shells
adpt deployment current            # print the deployment this shell sees
adpt whoami                        # active deployment + resolved settings
adpt --deployment prod recipes     # one-off override for a single command
adpt deployment remove staging     # delete it (and its API key)
```

### How switching works

`adpt deployment use` spawns a new shell with `$ADPT_DEPLOYMENT` exported.

If you want a switch that does propagate to fresh shells, add `--persist`
(or use `setup`, which always persists). Persisting writes the
file-level `active_deployment` in `~/.adpt/config.toml`.

Resolution order for every command:

1. `--deployment <name>` flag
2. `$ADPT_DEPLOYMENT` env var (your current shell)
3. `active_deployment` in the config file (the file-level default)

### Shell prompt integration

You can configure your shell prompt to show the active deployment by
printing the output of `adpt deployment current` in it.

## Configuration

### Env vars and `.env` overrides

Environment variables, from the shell or a `.env` file in the current
directory (or any parent), override the deployment selection and its
fields on a per-invocation basis:

- `ADPT_DEPLOYMENT`: pins the deployment to use (same as `--deployment`)
- `ADAPTIVE_BASE_URL`: overrides the base URL
- `ADAPTIVE_API_KEY`: overrides the keyring-stored key
- `DEFAULT_PROJECT`: overrides the default project

### Configuration file locations

| Platform    | Configuration File Path                                             |
| ----------- | ------------------------------------------------------------------- |
| **Linux**   | `~/.config/adpt/config.toml` or `$XDG_CONFIG_HOME/adpt/config.toml` |
| **macOS**   | `~/.adpt/config.toml`                                               |
| **Windows** | `%APPDATA%\adaptive-ml\adpt\config\config.toml`                     |

### Configuration file format

```toml
active_deployment = "prod"

[deployments.prod]
adaptive_base_url = "https://prod.adaptive.example"
default_project = "my-project"

[deployments.staging]
adaptive_base_url = "https://staging.adaptive.example"
```

API keys are stored per-deployment in the OS keyring, never in this file.
