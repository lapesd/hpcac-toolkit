from hpcac_cli.utils.logger import info_prompt


def prompt_text(text: str) -> str:
    info_prompt(text)
    value = input("> ")

    return value


def prompt_confirmation(text: str) -> bool:
    info_prompt(f"{text} (y/N):")
    value = input("> ").strip().lower()

    return value.startswith("y")
