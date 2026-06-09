# MeiLang Workflow Recipes

## Bootstrap a standalone workspace

```bash
mei-toolchain workspace init --standalone --source-root /path/to/workspace --materialize --tool cursor --json
```

## Create a new app

```bash
mei-toolchain workspace create-app my-app --source-root /path/to/workspace --scaffold --tool cursor --json
```

## Export packaged knowledge for an AI tool

```bash
mei-toolchain knowledge --surface author --source-root /path/to/workspace --include-content --json
mei-toolchain knowledge --surface author --source-root /path/to/workspace --topic author_profile --include-content --json
mei-toolchain knowledge --surface author --source-root /path/to/workspace --topic components --include-content --json
mei-toolchain knowledge --surface author --source-root /path/to/workspace --topic examples --include-content --json
```

## Describe and verify the local editor runtime

```bash
mei-toolchain editor-runtime describe --json
mei-toolchain editor-runtime doctor --json
```

## Validate a standalone app

```bash
mei-toolchain check --app my-app --source-root /path/to/workspace --json
```
