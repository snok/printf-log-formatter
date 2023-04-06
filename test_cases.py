too_many_arguments = logger.info("{}".format(1, 2, 3))
multiple_named = logger.info("{x} {x}".format(x=1))
formatting = logger.info("{x:02f} {x:03f}".format(x=1))
ordering = logger.info("{y} - {x} - {y} * {x} + {} = {}".format(5, 4, x=2, y=1))
