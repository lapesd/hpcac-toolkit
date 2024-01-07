import argparse

from tortoise import run_async

from hpcac_cli.db import init_db
from hpcac_cli.utils.logger import error, info
from hpcac_cli.commands.cluster import create_cluster, destroy_cluster


def main():
    info("Welcome to HPC@Cloud!")
    run_async(init_db())

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

    args = parser.parse_args()
    if args.command is None:
        parser.print_help()
    else:
        try:
            args.func()
        except Exception as e:
            error(e)

    # try:
    #    run_config = yaml_parser()
    # except Exception as e:
    #    error(e)
