too_many_arguments = logger.info("{}".format(1, 2, 3))

variable = """
x = 2
logger.info("{x}".format(x=x))
"""
multiple_named = logger.info("{x} {x}".format(x=1))
formatting = logger.info("{x:02f} {x:03f}".format(x=1))
ordering = logger.info("{y} - {x} - {y} * {x} + {} = {}".format(5, 4, x=2, y=1))
single_line_multiple = logger.info("{}".format(1)); logger.info("{}".format(1)); logger.info("{}".format(1))
multi_line = """
logger.info(
    "{}".format(1)
)
"""