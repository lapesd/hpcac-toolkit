import json
from typing import Optional

from colorama import init, Fore


init()


class Logger:
    COLOR_DICT = {
        "red": Fore.RED,
        "blue": Fore.BLUE,
        "cyan": Fore.CYAN,
        "green": Fore.GREEN,
        "white": Fore.WHITE,
        "black": Fore.BLACK,
        "yellow": Fore.YELLOW,
        "magenta": Fore.MAGENTA,
        "light_red": Fore.LIGHTRED_EX,
        "light_cyan": Fore.LIGHTCYAN_EX,
        "light_blue": Fore.LIGHTBLUE_EX,
        "light_white": Fore.LIGHTWHITE_EX,
        "light_green": Fore.LIGHTGREEN_EX,
        "light_black": Fore.LIGHTBLACK_EX,
        "light_yellow": Fore.LIGHTYELLOW_EX,
        "light_magenta": Fore.LIGHTMAGENTA_EX,
    }

    @staticmethod
    def _colorize(text: str, color: str) -> str:
        return f"{Logger.COLOR_DICT.get(color, Fore.WHITE)}{text}{Fore.RESET}"

    @classmethod
    def _log(
        cls,
        level: str,
        text: str,
        level_color: str,
        text_color: str,
        detail: Optional[str] = None,
    ):
        prefix = cls._colorize(level, level_color)
        message = cls._colorize(text, text_color)
        detail_msg = cls._colorize(detail, level_color)
        if detail is not None:
            print(f"[{prefix}] ({detail_msg}) {message}")
        else:
            print(f"[{prefix}] {message}")

    def info(self, text: str, detail: Optional[str] = None):
        self._log("INFO", text, "green", "light_green", detail)

    def error(self, text: str, detail: Optional[str] = None):
        self._log("ERROR", text, "red", "light_red", detail)

    def warning(self, text: str, detail: Optional[str] = None):
        self._log("WARNING", text, "yellow", "light_yellow", detail)

    def debug(self, text: str, detail: Optional[str] = None):
        self._log("DEBUG", text, "light_black", "light_black", detail)
