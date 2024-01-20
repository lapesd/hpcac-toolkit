import json

from colorama import init, Fore


init()


def log(text: str, color: str) -> None:
    color_dict = {
        "red": Fore.RED,
        "green": Fore.GREEN,
        "yellow": Fore.YELLOW,
        "blue": Fore.BLUE,
        "magenta": Fore.MAGENTA,
        "cyan": Fore.CYAN,
        "white": Fore.WHITE,
    }
    print(f"{color_dict.get(color, Fore.WHITE)}{text}{Fore.RESET}")


def error(text: str) -> None:
    print(f"[{Fore.RED}ERROR{Fore.RESET}] {Fore.LIGHTRED_EX}{text}{Fore.RESET}")


def warning(text: str) -> None:
    print(f"[{Fore.YELLOW}WARNING{Fore.RESET}] {Fore.LIGHTYELLOW_EX}{text}{Fore.RESET}")


def info(text: str) -> None:
    print(f"[{Fore.GREEN}INFO{Fore.RESET}] {Fore.WHITE}{text}{Fore.RESET}")


def info_remote(ip: str, text: str) -> None:
    print(
        f"[{Fore.GREEN}INFO{Fore.RESET}] [{Fore.LIGHTGREEN_EX}{ip}{Fore.RESET}] {Fore.WHITE}{text}{Fore.RESET}"
    )


def info_prompt(text: str) -> None:
    print(f"[{Fore.BLUE}PROMPT{Fore.RESET}] {Fore.LIGHTBLUE_EX}{text}{Fore.RESET}")


def info_terraform(text: str) -> None:
    print(
        f"{Fore.RESET}[{Fore.GREEN}INFO{Fore.RESET}] {Fore.RESET}[{Fore.MAGENTA}TF{Fore.RESET}] {Fore.LIGHTMAGENTA_EX}{text}{Fore.RESET}",
        end="",
    )


def print_map(map: dict) -> None:
    info("Your config is:")
    print(json.dumps(map, indent=4, sort_keys=True))
