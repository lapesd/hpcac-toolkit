from setuptools import setup, find_packages

setup(
    name="hpcac_cli",
    version="2.0",
    packages=find_packages(),
    entry_points={
        "console_scripts": [
            "hpcac = hpcac_cli.cli:main",
        ],
    },
)
