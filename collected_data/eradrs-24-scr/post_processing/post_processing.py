import csv
from pathlib import Path
import capture


INPUT_DIR = capture.OUTPUT_DIR + capture.EXP_PREFIX + "$.csv"
OUTPUT_FILE = capture.DATA_DIR + "final/" + capture.EXP_PREFIX + "$.csv"


def calculate_stdev(numbers: list[float]) -> float:
    """
    Calculates the standard deviation of a list of numbers.
    """
    n = len(numbers)
    average = sum(numbers) / n
    k = 0
    for i in numbers:
        k += (i - average) ** 2
    stdev = (k / (n - 1)) ** 0.5
    return stdev


def calculate_stdev_percentage(
    stdevs: list[float], total_timings: list[float]
) -> list[float]:
    """
    Calculates the percentage of the standard deviation in relation to the total time.
    """
    # stdev/total_timing = x%/100% -> x = 100*stdev/total_timing

    percentages = []
    for stdev, total_timing in zip(stdevs, total_timings):
        percentages.append(100 * stdev / total_timing)
    return percentages


def calculate_average_timing(numbers: list[float]) -> float:
    """
    Calculates the average time of a list of numbers.
    """
    return sum(numbers) / len(numbers)


def capture_total_timings(filename: str) -> list[float]:
    """
    Returns a list with the total times of each execution.
    """
    timings = []
    with open(filename, "r") as file:
        csv_reader = csv.reader(file)
        next(csv_reader)  # header
        for row in csv_reader:
            if row == []:
                continue
            timings.append(float(row[0]))
    return timings


def capture_non_idle_timings(filename: str) -> list[float]:
    """
    Returns a list with the non-idle timings of each execution.
    """
    timings = []
    with open(filename, "r") as file:
        csv_reader = csv.reader(file)
        next(csv_reader)  # header
        for row in csv_reader:
            if row == []:
                continue
            timings.append(float(row[1]))
    return timings


def capture_functions_timings(filename: str) -> list[list[float]]:
    """
    Returns a list with the timings of each function of each execution.
    """
    timings = [[] for _ in range(9)]
    with open(filename, "r") as file:
        csv_reader = csv.reader(file)
        next(csv_reader)  # header
        for row in csv_reader:
            if row == []:
                continue
            for i in range(2, 11):
                timings[i - 2].append(float(row[i]))
    return timings


def write_final(
    filename: str,
    average_total_timings: float,
    stdev_total_timings: float,
    porc_stdev_total_timings: float,
    non_idle_avg_timings: float,
    stdev_non_idle_timings: float,
    porc_stdev_non_idle_timings: float,
    avg_functions_timings: list[float],
    stdev_functions_timings: list[float],
    perc_stdev_functions_timings: list[float],
) -> None:
    """
    Write the results to a .csv file, with the following format:
    average_total_timings, stdev_total_timings
    non_idle_avg_timings, stdev_non_idle_timings
    avg_func1_timing, stdev_func1
    ...
    avg_func10_timing, stdev_func10
    """
    if not Path(filename).parent.exists():
        Path(filename).parent.mkdir()

    header_list = capture.HEADER.split(",")

    with open(filename, "w") as file:
        csv_writer = csv.writer(file)
        csv_writer.writerow(
            ["label", "average", "stdev", "porcentagem_stdev"]
        )
        csv_writer.writerow(
            [
                header_list[0],
                average_total_timings,
                stdev_total_timings,
                porc_stdev_total_timings,
            ]
        )
        csv_writer.writerow(
            [
                header_list[1],
                non_idle_avg_timings,
                stdev_non_idle_timings,
                porc_stdev_non_idle_timings,
            ]
        )
        for i in range(len(avg_functions_timings)):
            csv_writer.writerow(
                [
                    header_list[i + 2],
                    avg_functions_timings[i],
                    stdev_functions_timings[i],
                    perc_stdev_functions_timings[i],
                ]
            )


def main():
    """
    Executes the post-processing.
    """
    for i in range(1, 25):
        file = INPUT_DIR.replace("$", str(i))

        total_timings = capture_total_timings(file)
        timings_nao_idle = capture_non_idle_timings(file)
        timings_por_funcao = capture_functions_timings(file)

        average_total_timings = calculate_average_timing(total_timings)
        stdev_total_timings = calculate_stdev(total_timings)
        porc_stdev_total_timings = calculate_stdev_percentage(
            [stdev_total_timings], [average_total_timings]
        )[0]

        non_idle_avg_timings = calculate_average_timing(timings_nao_idle)
        stdev_non_idle_timings = calculate_stdev(timings_nao_idle)
        porc_stdev_non_idle_timings = calculate_stdev_percentage(
            [stdev_non_idle_timings], [non_idle_avg_timings]
        )[0]

        avg_functions_timings = []
        stdev_functions_timings = []
        for j in timings_por_funcao:
            avg_functions_timings.append(calculate_average_timing(j))
            stdev_functions_timings.append(calculate_stdev(j))
        perc_stdev_functions_timings = calculate_stdev_percentage(
            stdev_functions_timings, avg_functions_timings
        )

        write_final(
            OUTPUT_FILE.replace("$", str(i)),
            average_total_timings,
            stdev_total_timings,
            porc_stdev_total_timings,
            non_idle_avg_timings,
            stdev_non_idle_timings,
            porc_stdev_non_idle_timings,
            avg_functions_timings,
            stdev_functions_timings,
            perc_stdev_functions_timings,
        )


if __name__ == "__main__":
    main()
