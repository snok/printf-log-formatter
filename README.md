<img src="https://raw.githubusercontent.com/sondrelg/printf-log-formatter/main/logo.svg?token=GHSAT0AAAAAACAOR4AAQVLI3YMI4IZKDAYCZBS5KOA&sanitize=true" alt="logo" width="110" align="right">

# printf-log-formatter

Automatically convert f-strings and `str.format()` syntax to printf style strings.

In other words,



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

[This article](https://blog.pilosus.org/posts/2020/01/24/python-f-strings-in-logging/) explains it well.

tl;dr: It fixes Sentry log integration issues.

## Installation

Install with pre-commit, using:

```yaml
- repo: https://github.com/sondrelg/printf-log-formatter
  rev: ""
  hooks:
    - id: printf-log-formatter
      args:
        - --log-level=error
        - --quotes=single  # or double
```

or possibly soon:

```yaml
- repo: https://github.com/sondrelg/pre-commit-binary
  rev: ""
  hooks:
    - id: pre-commit-binary
      alias: printf-log-formatter
      args:
        # Specify binary URLs for each platform/arch combination needed
        - |
        --urls={
          "darwin-x86_64":  "https://github.com/sondrelg/printf-log-formatter/releases/v1/darwin-x64_86.tar.gz",
          "darwin-arm64":  "https://github.com/sondrelg/printf-log-formatter/releases/v1/darwin-arm64.tar.gz",
          "linux-x64_64":  "https://github.com/sondrelg/printf-log-formatter/releases/v1/linux-x64_86.zip"
        }
        # Remaining arguments passed to the binary
        - --log-level=error
        - --quotes=single
```
