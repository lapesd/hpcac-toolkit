import sys
import shutil
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent

CLUSTER_CONFIG_DIR = BASE_DIR / "cluster_configs"

TASKS_DIR = BASE_DIR / "tasks_configs"

BAK_DIR = BASE_DIR / "bak"

TMP_FILE = BASE_DIR / "setup_tmp.txt"

CLUSTER_CONFIG_PATTERN = "cluster_config$.yaml"

TASK_PATTERN = "tasks_config$.yaml"


def exit_program(with_error: bool = False):
    exit(with_error)


def setup():
    if not BAK_DIR.is_dir():
        BAK_DIR.mkdir()

    if not TMP_FILE.is_file():
        with open(TMP_FILE, "w") as f:
            f.write("1")


def set_cluster_config(cluster_number: int):
    if not CLUSTER_CONFIG_DIR.is_dir():
        print("No cluster config dir was found.")
        exit_program(True)

    original = CLUSTER_CONFIG_PATTERN.replace("$", "")

    if Path(original).is_file():
        # copy file to bak dir
        print(f"copying {original} to bak dir...")
        bak_file = str(BAK_DIR.joinpath(original).absolute())

        # if bak_file exists, move the file as bak_file (copy)
        while Path(bak_file).is_file():
            bak_file += "(1)"

        shutil.move(original, bak_file)

    source_name = CLUSTER_CONFIG_PATTERN.replace("$", str(cluster_number))

    print(f"copying {source_name} to {original}...")
    source = CLUSTER_CONFIG_DIR.joinpath(source_name)
    shutil.copyfile(source, original)


def set_mpi_run(mpi_run_number: int):
    if not TASKS_DIR.is_dir():
        print("No mpi_run dir was found.")
        exit_program(True)

    original = TASK_PATTERN.replace("$", "")

    if Path(original).is_file():
        # copy file to bak dir
        print(f"copying {original} to bak dir...")
        bak_file = str(BAK_DIR.joinpath(original).absolute())

        # if bak_file exists, move the file as bak_file (copy)
        while Path(bak_file).is_file():
            bak_file += "(1)"

        shutil.move(original, bak_file)

    with open(TMP_FILE, "r") as f:
        mpi_run_number = int(f.read())

    source_name = TASK_PATTERN.replace("$", str(mpi_run_number))

    print(f"copying {source_name} to {original}...")
    source = TASKS_DIR.joinpath(source_name)
    shutil.copyfile(source, original)


def print_help():
    print("Usage:")
    print("\tpython setup.py <option>")
    print("Options:")
    print("\thelp  : print this help")
    print("\tswitch: switch cluster config and tasks config")
    print("\tclean : clean bak dir and tmp file")


def clean():
    if BAK_DIR.is_dir():
        print("removing bak dir...")
        shutil.rmtree(BAK_DIR)
    if TMP_FILE.is_file():
        print("removing tmp file...")
        TMP_FILE.unlink()


def switch():
    q1 = len(list(CLUSTER_CONFIG_DIR.glob(CLUSTER_CONFIG_PATTERN.replace("$", "*"))))
    q2 = len(list(TASKS_DIR.glob(TASK_PATTERN.replace("$", "*"))))

    if q1 == 0 or q2 == 0:
        print("No cluster config or mpi run was found.")
        exit_program(True)

    if q1 != q2:
        print("Quantity of cluster config and task config files are different.")
        exit_program(True)

    with open(TMP_FILE, "r") as f:
        number = int(f.read())

    set_cluster_config(number)
    set_mpi_run(number)

    number += 1
    if number > q1:
        number = 1

    with open(TMP_FILE, "w") as f:
        f.write(str(number))


if __name__ == "__main__":
    if len(sys.argv) == 2:
        option = sys.argv[1].lower()
        if option == "help":
            print_help()
            exit_program()
        elif option == "switch":
            setup()
            switch()
            print("Done.")
        elif option == "clean":
            clean()
            print("Done.")
        else:
            print("Invalid option.")
            print_help()
            exit_program(True)
    else:
        print_help()
        exit_program(True)
