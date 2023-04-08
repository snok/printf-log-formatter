import logging

logger = logging.getLogger(__name__)

x = 1
y = 2

name = "Nils"

logger.info(f"{1} {x} {y:02f}")
logger.debug("{} {x} {y:02f}".format(1, x=2, y=3))
logger.info(f"{name}")
logger.info("{}".format(name))
logger.info("{name}".format(name=name))
