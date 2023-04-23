<a href="https://github.com/sondrelg/printf-log-formatter"><img src="https://raw.githubusercontent.com/sondrelg/printf-log-formatter/main/logo.svg?token=GHSAT0AAAAAACAOR4AAQVLI3YMI4IZKDAYCZBS5KOA&sanitize=true" alt="logo" width="110" align="right"></a>

# printf-log-formatter

Automatically convert f-strings and `str.format()` syntax to printf-style strings.

In other words, this syntax

```python
logger.error(f"{1}")
logger.error("{}".format(1))
logger.error("{foo}".format(foo=1))
```

is changed to

```python
logger.error("%s", 1)
logger.error("%s", 1)
logger.error("%s", 1)
```


## Motivation

Why would we want to do this? [This article](https://blog.pilosus.org/posts/2020/01/24/python-f-strings-in-logging/) explains it pretty well.

Mainly it's useful for Python projects using [Sentry](https://sentry.io)'s log integration.

## Installation

You have two options for running this pre-commit hook:


### Python hook

If you would like to install this using Python, run:

```shell
pip install printf-log-formatter
```

then set the pre-commit hook up using:

```yaml
- repo: local
  hooks:
  - id: printf-log-formatter
    name: printf-log-formatter
    entry: printf-log-formatter
    language: system
    types: [ python ]
    args:
      - --log-level=error
```


### Rust hook

If you're happy to compile the Rust version, you can use:

```yaml
- repo: https://github.com/sondrelg/printf-log-formatter
  rev: ''
  hooks:
    - id: printf-log-formatter
      args:
        - --log-level=error
```

## I just want to downgrade loggers once

The Rust binary or Python package can also be run directly, like this:

```shell
printf-log-formatter $(find . -name "*.py") --log-level error
```
