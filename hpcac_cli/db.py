from tortoise import Tortoise


TORTOISE_ORM = {
    "connections": {"default": "postgres://local:local@127.0.0.1:5437/postgres"},
    "apps": {
        "models": {
            "models": [
                "hpcac_cli.models.cluster",
                "hpcac_cli.models.task",
                "aerich.models",
            ],
            "default_connection": "default",
        },
    },
}


async def init_db():
    await Tortoise.init(
        db_url=TORTOISE_ORM["connections"]["default"],
        modules={"models": TORTOISE_ORM["apps"]["models"]["models"]},
    )
    await Tortoise.generate_schemas()
