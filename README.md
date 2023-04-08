# printf-log-formatter

Automatically converts f-strings and other python string formats to use printf style strings. In other words, this plugin will convert

```python
what = "nightmare"
logger.info(f"{name}")
logger.info("{}".format(name))
logger.info("{name}".format(name=name))
```

to 

```python
logger.info("%s", name) 
logger.info("%s", name) 
logger.info("%s", name) 
```


## Motivations
https://blog.pilosus.org/posts/2020/01/24/python-f-strings-in-logging/

```yaml
- repo: https://github.com/sondrelg/printf-log-formatter
  hooks:
    - id: printf-log-formatter
      args:
        - --quotes=single  # or double
```

## TODO:

- [ ] Add tests for .format
- [ ] Add support for f-string
- [ ] Sprinkle inn appropriate logging
- [ ] Add appropriate tests
- [ ] Test on Cloud
- [ ] Make public