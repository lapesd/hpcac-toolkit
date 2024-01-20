import argparse

import asyncio

from hpcac_cli.db import init_db
from hpcac_cli.utils.logger import error, info
from hpcac_cli.commands.clusters import create_cluster, destroy_cluster
from hpcac_cli.commands.tasks import run_tasks


async def main_async():
    info("Welcome to HPC@Cloud!")
    await init_db()

    parser = argparse.ArgumentParser(description="HPC@Cloud CLI tool")
    subparsers = parser.add_subparsers(dest="command")

    parser_create = subparsers.add_parser(
        "create-cluster",
        help="Create a new cluster",
    )
    parser_create.set_defaults(func=create_cluster)

    parser_destroy = subparsers.add_parser(
        "destroy-cluster",
        help="Destroy an existing cluster",
    )
    parser_destroy.set_defaults(func=destroy_cluster)

    parser_create = subparsers.add_parser(
        "run-tasks",
        help="Run tasks in a cluster",
    )
    parser_create.set_defaults(func=run_tasks)

    args = parser.parse_args()

    if args.command is None:
        parser.print_help()
    else:
        try:
            # Check if the command function is asynchronous
            if asyncio.iscoroutinefunction(args.func):
                await args.func()
            else:
                args.func()
        except KeyboardInterrupt:
            error("\nCommand CANCELLED by the user.")
        except Exception as e:
            error(e)


def main():
    asyncio.run(main_async())


if __name__ == "__main__":
    main()
