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
adpt deployment list               # see them all; the active one is marked
adpt deployment use staging        # switch
adpt deployment current            # print just the active name
adpt whoami                        # active deployment + resolved settings
adpt --deployment prod recipes     # one-off override without switching
adpt deployment remove staging     # delete it (and its API key)
```

Legacy flat configs (pre-FE-26) are migrated automatically into a
`default` deployment on first run.

### Shell prompt integration

`adpt deployment current` is fast and pipe-friendly — wrap it in your
prompt to always see which deployment you're talking to. Bash/zsh:

```sh
# in ~/.bashrc or ~/.zshrc
adpt_prompt() {
  local d
  d=$(adpt deployment current 2>/dev/null) && [ -n "$d" ] && printf ' (adpt:%s)' "$d"
}
PS1='\u@\h \w$(adpt_prompt)\$ '   # bash
# zsh:  PROMPT='%n@%m %~$(adpt_prompt)%# '
```

Fish:

```fish
function fish_right_prompt
  set -l d (adpt deployment current 2>/dev/null)
  test -n "$d"; and echo "adpt:$d"
end
```

Starship/oh-my-posh users can point a `custom` segment at
`adpt deployment current`.

## Configuration

### Env file overrides

Environment variables in a `.env` file in the current directory (or any
parent) override individual fields of the active deployment:

- `ADAPTIVE_BASE_URL` — overrides the base URL
- `ADAPTIVE_API_KEY` — overrides the keyring-stored key
- `DEFAULT_PROJECT` — overrides the default project

This makes it easy to pin a repo to a particular Adaptive instance without
switching your global active deployment.

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
