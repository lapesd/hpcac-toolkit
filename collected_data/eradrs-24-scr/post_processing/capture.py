import os
import sys
import csv

HEADER = "tempo total,tempo não idle,SCR_Init,SCR_Have_restart,SCR_Start_restart,SCR_Route_file,SCR_Complete_restart,SCR_Need_checkpoint,SCR_Start_output,SCR_Complete_output,SCR_Finalize"

DATA_DIR = "../collected_data/"
OUTPUT_DIR = DATA_DIR + "mid-processing/"
EXP_PREFIX = "exp"


def capture_exp(exp_file: str) -> list[float]:
    cont = 0
    t_time_list = []

    with open(exp_file, "r") as file:
        last_lines = file.readlines()[-14:]

    for line in last_lines:
        cont += 1

        if not line.strip():
            continue

        elif cont == 3:
            t_terminate_instances = float(line.split()[2])

        elif cont > 4:
            t_time = float(line.split()[3])
            t_time_list.append(t_time)

    timings = [t_terminate_instances, *t_time_list]
    return timings


def write_list_to_file(output_file: str, timings: list[float]):
    with open(output_file, "a") as file:
        phrase = ",".join([str(x) for x in timings]) + "\n"
        file.write(phrase)


def write_header_to_file(output_file: str):
    with open(output_file, "w") as file:
        file.write(HEADER + "\n")


def print_help_and_exit(with_error: bool = False):
    print("Usage: python3 capture.py <dir_path>")
    sys.exit(with_error)


def capture_extra_timings(
    file: str, label_prefix: str, exp_number: int
) -> list[list[float, float]]:
    labels = {f"{label_prefix} {exp_number}-{i}": [0, 0] for i in range(1, 6)}
    extra_timings_list = []
    with open(file) as fd:
        csv_reader = csv.reader(fd)
        next(csv_reader)  # header
        for line in csv_reader:
            task_tag = line[0].strip()
            was_completed = line[9].strip() == "True"
            if task_tag in labels:
                if not was_completed:
                    print(f"Task {task_tag} was not completed")
                    continue

                time_spent_executing_task = float(line[14])
                time_spent_setting_up_task = float(line[11])
                non_idle_time = time_spent_setting_up_task + time_spent_executing_task

                time_spent_spawning_cluster = float(line[10])
                total_time = float(line[15]) - time_spent_spawning_cluster

                labels[task_tag] = [total_time, non_idle_time]

    extra_timings_list = list(labels.values())
    return extra_timings_list


def main():
    qtd_args = len(sys.argv)
    if qtd_args == 1:
        print_help_and_exit()
    elif qtd_args != 2:
        print_help_and_exit(True)

    dir_path = sys.argv[1]
    if dir_path.endswith("/"):
        dir_path = dir_path[:-1]

    if not os.path.exists(OUTPUT_DIR):
        os.mkdir(OUTPUT_DIR)

    for i in range(1, 25):
        filename = f"{dir_path}/{EXP_PREFIX}{i}"
        output_file = f"{OUTPUT_DIR}/{EXP_PREFIX}{i}.csv"

        write_header_to_file(output_file)

        # extra timings: total_time, non_idle_time
        extra_data_file = f"{dir_path}/task_results.csv"
        extra_timings = capture_extra_timings(extra_data_file, EXP_PREFIX, i)
        for j in range(1, 6):
            exp_file = f"{filename}-{j}.txt"

            timings = [*extra_timings[j - 1]]

            if not os.path.exists(exp_file):
                print(f"Experiment number {i}-{j} ({exp_file}) not found")
            else:
                t_terminate_instances, *t_timings = capture_exp(exp_file)
                timings[
                    1
                ] -= t_terminate_instances  # remove t_terminate_instances from non_idle_time
                timings.extend(t_timings)

                write_list_to_file(output_file, timings)

        print(f"Captured experiment number {i}")

    print("Done")


if __name__ == "__main__":
    main()
