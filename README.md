# adpt

A command line tool for interacting with the Adaptive Platform.

## Installation

```sh
cargo install adpt
```

Or on ARM-based macs brew can be used:

```sh
brew install adaptive-ml/homebrew-tap/adpt
```

Once installed an API key must be specified for use. This can be done using the
`ADAPTIVE_API_KEY` environment variable, or alternatively stored in your
operating system's keyring using the below command:

```sh
adpt set-apt-key
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
usecase is configured.

## Usage

### Specifying the use case

Most commands require a `--usecase` option to specify the use case:

```sh
adpt recipes --usecase my-usecase
```

However to avoid specifying this every time, the `DEFAULT_USECASE` environment
variable or the `default_usecase` configuration file option.:

### Setting API Key

Store your API key in the system keyring:

```sh
adpt set-api-key <your-api-key>
```

### Running Recipes

Run a recipe by its ID or key:

```sh
adpt run <recipe-key-or-id>
```

Run a recipe with parameters from a JSON file:

```sh
adpt run my-recipe --parameters params.json
```

Run a recipe with custom settings:

```sh
adpt run my-recipe --name "My Custom Run" --compute-pool gpu-pool --num-gpus 4
```

### Publishing Recipes

Publish a recipe from a file or a directory containing a `main.py`:

```sh
adpt publish /path/to/recipe-directory
```

Publish a recipe with a custom name and key:

```sh
adpt publish /path/to/recipe --name "My Recipe" --key my-recipe-key
```

### Listing Recipes

List all available recipes:

```sh
adpt recipes
```

### Uploading datasets

Upload a dataset:

```sh
adpt upload /path/to/dataset
```

### Monitoring Jobs

Get the status of a specific job:

```sh
adpt job <job-id>
```

Follow a job's progress until completion:

```sh
adpt job <job-id> --follow
```

## Configuration

### Env file

Envionrment variables may be specified using a `.env` file in a parent folder.

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
# Default use case for operations
default_use_case = "my-usecase"

# Base URL for the Adaptive platform
adaptive_base_url = "https://your-adaptive-instance.com"
```

### API Key Storage

The API key can be provided in two ways (in order of priority):

1. **Environment Variable**: Set `ADAPTIVE_API_KEY` environment variable
2. **System Keyring**: Store securely using `adpt set-api-key <your-key>`
