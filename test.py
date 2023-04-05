import logging

logger = logging.getLogger(__name__)

x, y, z = 1, 2, 3

logger.info(f"name {x} {y} {z}")
logger.info(f"name {y} {z} {x}")
logger.info(f"name {y} {z}")
logger.info("name {x}".format(x=x))
logger.info("name {}".format(x))
