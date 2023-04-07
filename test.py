import logging

logger = logging.getLogger(__name__)

zz = 2

logger.info(
    "{y:02f} - {x} - {y} * {x} + {} = {}".format(
        5,
        zz,
        x=2,
        y=1
    )
)
logger.info("{y:02f} - {x} - {y} * {x} + {} = {}".format(5,
        4,
        x=2,
        y=1
    )
)
logger.info("{y:02f} - {x} - {y} * {x} + {} = {}".format(5, 4, x=2, y=1))

# # Actual
# lineno 6
# start co 4

# End_row 11
# End_col 5
#
# # Real (args.0.Call)
# lineno: 7
# col_offset: 4
# end_col_offset: 5
# end_lineno: 12
