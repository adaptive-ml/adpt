# Command-Line Help for `adpt`

This document contains the help content for the `adpt` command-line program.

**Command Overview:**

* [`adpt`↴](#adpt)
* [`adpt cancel`↴](#adpt-cancel)
* [`adpt config`↴](#adpt-config)
* [`adpt job`↴](#adpt-job)
* [`adpt jobs`↴](#adpt-jobs)
* [`adpt models`↴](#adpt-models)
* [`adpt upload`↴](#adpt-upload)
* [`adpt publish`↴](#adpt-publish)
* [`adpt recipes`↴](#adpt-recipes)
* [`adpt run`↴](#adpt-run)
* [`adpt schema`↴](#adpt-schema)
* [`adpt set-api-key`↴](#adpt-set-api-key)
* [`adpt role`↴](#adpt-role)
* [`adpt role create`↴](#adpt-role-create)
* [`adpt role describe`↴](#adpt-role-describe)
* [`adpt role list`↴](#adpt-role-list)
* [`adpt user`↴](#adpt-user)
* [`adpt user create`↴](#adpt-user-create)
* [`adpt user delete`↴](#adpt-user-delete)
* [`adpt user describe`↴](#adpt-user-describe)
* [`adpt user list`↴](#adpt-user-list)
* [`adpt team`↴](#adpt-team)
* [`adpt team create`↴](#adpt-team-create)
* [`adpt team list`↴](#adpt-team-list)

## `adpt`

A tool interacting with the Adaptive platform

**Usage:** `adpt <COMMAND>`

###### **Subcommands:**

* `cancel` — Cancel a job
* `config` — Configure adpt interactively
* `job` — Inspect job
* `jobs` — List currently running jobs
* `models` — List models
* `upload` — Upload dataset
* `publish` — Upload recipe
* `recipes` — List recipes
* `run` — Run recipe
* `schema` — Display the schema for inputs for a recipe
* `set-api-key` — Store your API key in the OS keyring
* `role` — Manage roles
* `user` — Manage users
* `team` — Manage teams



## `adpt cancel`

Cancel a job

**Usage:** `adpt cancel <ID>`

###### **Arguments:**

* `<ID>`



## `adpt config`

Configure adpt interactively

**Usage:** `adpt config`



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



## `adpt set-api-key`

Store your API key in the OS keyring

**Usage:** `adpt set-api-key <API_KEY>`

###### **Arguments:**

* `<API_KEY>`



## `adpt role`

Manage roles

**Usage:** `adpt role <COMMAND>`

###### **Subcommands:**

* `create` — Create a new role
* `describe` — Describe a role
* `list` — List all roles



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
* `list` — List all teams



## `adpt team create`

Create a new team

**Usage:** `adpt team create [OPTIONS] <NAME>`

###### **Arguments:**

* `<NAME>` — Team name

###### **Options:**

* `-k`, `--key <KEY>` — Team key (auto-generated from name if not provided)



## `adpt team list`

List all teams

**Usage:** `adpt team list`



<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>

