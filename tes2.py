import logging

logger = logging.getLogger(__name__)

x = "sondre"

logger.info(f"name {x}")
logger.info("name {}".format(x))
logger.info("name {x}".format(x=x))
logger.info("name %s", x)
