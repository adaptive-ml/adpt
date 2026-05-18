# Command-Line Help for `adpt`

This document contains the help content for the `adpt` command-line program.

**Command Overview:**

* [`adpt`↴](#adpt)
* [`adpt cancel`↴](#adpt-cancel)
* [`adpt deployment`↴](#adpt-deployment)
* [`adpt deployment setup`↴](#adpt-deployment-setup)
* [`adpt deployment list`↴](#adpt-deployment-list)
* [`adpt deployment show`↴](#adpt-deployment-show)
* [`adpt deployment use`↴](#adpt-deployment-use)
* [`adpt deployment current`↴](#adpt-deployment-current)
* [`adpt deployment remove`↴](#adpt-deployment-remove)
* [`adpt whoami`↴](#adpt-whoami)
* [`adpt job`↴](#adpt-job)
* [`adpt jobs`↴](#adpt-jobs)
* [`adpt models`↴](#adpt-models)
* [`adpt upload`↴](#adpt-upload)
* [`adpt publish`↴](#adpt-publish)
* [`adpt recipes`↴](#adpt-recipes)
* [`adpt run`↴](#adpt-run)
* [`adpt schema`↴](#adpt-schema)
* [`adpt role`↴](#adpt-role)
* [`adpt role create`↴](#adpt-role-create)
* [`adpt role describe`↴](#adpt-role-describe)
* [`adpt role list`↴](#adpt-role-list)
* [`adpt role add-permission`↴](#adpt-role-add-permission)
* [`adpt role remove-permission`↴](#adpt-role-remove-permission)
* [`adpt user`↴](#adpt-user)
* [`adpt user create`↴](#adpt-user-create)
* [`adpt user delete`↴](#adpt-user-delete)
* [`adpt user describe`↴](#adpt-user-describe)
* [`adpt user list`↴](#adpt-user-list)
* [`adpt team`↴](#adpt-team)
* [`adpt team create`↴](#adpt-team-create)
* [`adpt team add-member`↴](#adpt-team-add-member)
* [`adpt team remove-member`↴](#adpt-team-remove-member)
* [`adpt team list`↴](#adpt-team-list)

## `adpt`

A tool interacting with the Adaptive platform

**Usage:** `adpt [OPTIONS] <COMMAND>`

###### **Subcommands:**

* `cancel` — Cancel a job
* `deployment` — Manage Adaptive deployments
* `whoami` — Show the active deployment and its resolved settings
* `job` — Inspect job
* `jobs` — List currently running jobs
* `models` — List models
* `upload` — Upload dataset
* `publish` — Upload recipe
* `recipes` — List recipes
* `run` — Run recipe
* `schema` — Display the schema for inputs for a recipe
* `role` — Manage roles
* `user` — Manage users
* `team` — Manage teams

###### **Options:**

* `--deployment <DEPLOYMENT>` — Use a specific deployment for this invocation, overriding the active one



## `adpt cancel`

Cancel a job

**Usage:** `adpt cancel <ID>`

###### **Arguments:**

* `<ID>`



## `adpt deployment`

Manage Adaptive deployments

**Usage:** `adpt deployment <COMMAND>`

###### **Subcommands:**

* `setup` — Create or edit a deployment interactively
* `list` — List all configured deployments
* `show` — Show details for a deployment
* `use` — Set the active deployment
* `current` — Print just the active deployment name (for shell prompts)
* `remove` — Remove a deployment and its stored API key



## `adpt deployment setup`

Create or edit a deployment interactively

**Usage:** `adpt deployment setup [OPTIONS] [NAME]`

###### **Arguments:**

* `<NAME>` — Deployment name. If omitted, edits the active deployment

###### **Options:**

* `--url <URL>` — Base URL (for non-interactive setup, e.g. in CI)
* `--default-project <DEFAULT_PROJECT>` — Default project (for non-interactive setup)
* `--api-key <API_KEY>` — API key (for non-interactive setup)



## `adpt deployment list`

List all configured deployments

**Usage:** `adpt deployment list`



## `adpt deployment show`

Show details for a deployment

**Usage:** `adpt deployment show [NAME]`

###### **Arguments:**

* `<NAME>` — Deployment name. Defaults to the active deployment



## `adpt deployment use`

Set the active deployment

**Usage:** `adpt deployment use <NAME>`

###### **Arguments:**

* `<NAME>` — Deployment name to activate



## `adpt deployment current`

Print just the active deployment name (for shell prompts)

**Usage:** `adpt deployment current`



## `adpt deployment remove`

Remove a deployment and its stored API key

**Usage:** `adpt deployment remove [OPTIONS] <NAME>`

###### **Arguments:**

* `<NAME>` — Deployment name to remove

###### **Options:**

* `-f`, `--force` — Allow removing the active deployment



## `adpt whoami`

Show the active deployment and its resolved settings

**Usage:** `adpt whoami`



## `adpt job`

Inspect job

**Usage:** `adpt job [OPTIONS] <ID>`

###### **Arguments:**

* `<ID>`

###### **Options:**

* `-f`, `--follow` — Follow job status updates until completion



## `adpt jobs`

List currently running jobs

**Usage:** `adpt jobs`



## `adpt models`

List models

**Usage:** `adpt models [OPTIONS]`

###### **Options:**

* `-p`, `--project <PROJECT>`
* `-a`, `--all` — List all models in the global model registry



## `adpt upload`

Upload dataset

**Usage:** `adpt upload [OPTIONS] <DATASET>`

###### **Arguments:**

* `<DATASET>`

###### **Options:**

* `-p`, `--project <PROJECT>`
* `-n`, `--name <NAME>` — Dataset name



## `adpt publish`

Upload recipe

**Usage:** `adpt publish [OPTIONS] <RECIPE>`

###### **Arguments:**

* `<RECIPE>`

###### **Options:**

* `-p`, `--project <PROJECT>`
* `-n`, `--name <NAME>` — Recipe name
* `-k`, `--key <KEY>` — Recipe key
* `-e`, `--entrypoint <ENTRYPOINT>` — Custom entrypoint file
* `-c`, `--entrypoint-config <ENTRYPOINT_CONFIG>` — Custom config entrypoint file
* `-f`, `--force` — Update existing recipe if it exists



## `adpt recipes`

List recipes

**Usage:** `adpt recipes [OPTIONS]`

###### **Options:**

* `-p`, `--project <PROJECT>`



## `adpt run`

Run recipe

**Usage:** `adpt run [OPTIONS] <RECIPE> [-- <ARGS>...]`

###### **Arguments:**

* `<RECIPE>` — Recipe ID or key
* `<ARGS>`

###### **Options:**

* `-p`, `--project <PROJECT>`
* `--parameters <PARAMETERS>` — A file containing a JSON object of parameters for the recipe
* `-n`, `--name <NAME>` — The name of the run
* `-c`, `--compute-pool <COMPUTE_POOL>` — The compute pool to run the recipe on
* `-g`, `--gpus <GPUS>` — The number of GPUs to run the recipe on



## `adpt schema`

Display the schema for inputs for a recipe

**Usage:** `adpt schema [OPTIONS] <RECIPE>`

###### **Arguments:**

* `<RECIPE>`

###### **Options:**

* `-p`, `--project <PROJECT>`



## `adpt role`

Manage roles

**Usage:** `adpt role <COMMAND>`

###### **Subcommands:**

* `create` — Create a new role
* `describe` — Describe a role
* `list` — List all roles
* `add-permission` — Add permissions to a role
* `remove-permission` — Remove permissions from a role



## `adpt role create`

Create a new role

**Usage:** `adpt role create [OPTIONS] --permissions <PERMISSIONS>... <NAME>`

###### **Arguments:**

* `<NAME>` — Role name

###### **Options:**

* `-k`, `--key <KEY>` — Role key (auto-generated from name if not provided)
* `-p`, `--permissions <PERMISSIONS>` — Permissions to assign to the role



## `adpt role describe`

Describe a role

**Usage:** `adpt role describe <ID_OR_KEY>`

###### **Arguments:**

* `<ID_OR_KEY>` — Role ID (UUID) or key



## `adpt role list`

List all roles

**Usage:** `adpt role list`



## `adpt role add-permission`

Add permissions to a role

**Usage:** `adpt role add-permission <ROLE> <PERMISSIONS>...`

###### **Arguments:**

* `<ROLE>` — Role ID or key
* `<PERMISSIONS>` — Permissions to add



## `adpt role remove-permission`

Remove permissions from a role

**Usage:** `adpt role remove-permission <ROLE> <PERMISSIONS>...`

###### **Arguments:**

* `<ROLE>` — Role ID or key
* `<PERMISSIONS>` — Permissions to remove



## `adpt user`

Manage users

**Usage:** `adpt user <COMMAND>`

###### **Subcommands:**

* `create` — Create a new user
* `delete` — Delete a user
* `describe` — Describe a user
* `list` — List all users



## `adpt user create`

Create a new user

**Usage:** `adpt user create [OPTIONS] <NAME>`

###### **Arguments:**

* `<NAME>` — User name

###### **Options:**

* `-e`, `--email <EMAIL>` — User email (required for human users)
* `-t`, `--user-type <USER_TYPE>` — User type

  Default value: `human`

  Possible values: `human`, `system`




## `adpt user delete`

Delete a user

**Usage:** `adpt user delete <ID_OR_EMAIL>`

###### **Arguments:**

* `<ID_OR_EMAIL>` — User ID or email



## `adpt user describe`

Describe a user

**Usage:** `adpt user describe <ID_OR_EMAIL>`

###### **Arguments:**

* `<ID_OR_EMAIL>` — User ID or email



## `adpt user list`

List all users

**Usage:** `adpt user list`



## `adpt team`

Manage teams

**Usage:** `adpt team <COMMAND>`

###### **Subcommands:**

* `create` — Create a new team
* `add-member` — Add a user to a team
* `remove-member` — Remove a user from a team
* `list` — List all teams



## `adpt team create`

Create a new team

**Usage:** `adpt team create [OPTIONS] <NAME>`

###### **Arguments:**

* `<NAME>` — Team name

###### **Options:**

* `-k`, `--key <KEY>` — Team key (auto-generated from name if not provided)



## `adpt team add-member`

Add a user to a team

**Usage:** `adpt team add-member <USER> <TEAM> <ROLE>`

###### **Arguments:**

* `<USER>` — User ID or email
* `<TEAM>` — Team ID or key
* `<ROLE>` — Role ID or key



## `adpt team remove-member`

Remove a user from a team

**Usage:** `adpt team remove-member <USER> <TEAM>`

###### **Arguments:**

* `<USER>` — User ID or email
* `<TEAM>` — Team ID or key



## `adpt team list`

List all teams

**Usage:** `adpt team list`



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>

