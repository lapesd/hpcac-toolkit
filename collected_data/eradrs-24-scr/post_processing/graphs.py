import matplotlib.pyplot as plt
import numpy as np
import csv
import post_processing as pp
import typing as tp

# experiments:
# 1: t3.2xlarge  , 4 nodos, partner
# 2: t3.2xlarge  , 4 nodos, xor
# 3: t3.2xlarge  , 4 nodos, rs
#
# 4: t3.2xlarge  , 8 nodos, partner
# 5: t3.2xlarge  , 8 nodos, xor
# 6: t3.2xlarge  , 8 nodos, rs
#
# 7: c5.2xlarge  , 4 nodos, partner
# 8: c5.2xlarge  , 4 nodos, xor
# 9: c5.2xlarge  , 4 nodos, rs
#
# 10: c5.2xlarge , 8 nodos, partner
# 11: c5.2xlarge , 8 nodos, xor
# 12: c5.2xlarge , 8 nodos, rs
#
# 13: c6i.2xlarge, 4 nodos, partner
# 14: c6i.2xlarge, 4 nodos, xor
# 15: c6i.2xlarge, 4 nodos, rs
#
# 16: c6i.2xlarge, 8 nodos, partner
# 17: c6i.2xlarge, 8 nodos, xor
# 18: c6i.2xlarge, 8 nodos, rs
#
# 19: i3.2xlarge , 4 nodos, partner
# 20: i3.2xlarge , 4 nodos, xor
# 21: i3.2xlarge , 4 nodos, rs
#
# 22: i3.2xlarge , 8 nodos, partner
# 23: i3.2xlarge , 8 nodos, xor
# 24: i3.2xlarge , 8 nodos, rs

Experiment = tp.Tuple[tp.Tuple[str, float, float]]

Strategy_name = tp.NewType("Strategy_name", str)
Total_timings = tp.List[int]
Idle_timings = tp.List[int | str]  # lista vazia -> str (PEP 484)
Total_timings_stdev = tp.List[int]
Idle_timings_stdev = tp.List[int]

# Indices: 0 - t3.2xlarge, 1 - c5.2xlarge, 2 - c6i.2xlarge, 3 - i3.2xlarge
Cluster_results = tp.Tuple[
    Strategy_name,
    Total_timings,
    Idle_timings,
    Total_timings_stdev,
    Idle_timings_stdev,
]


DATA: list[Cluster_results] = [
    ["Sem Faltas", [181, 89, 81, 90], [0, 0, 0, 0]],
    ["ULFM", [537, 385, 341, 380], [327, 263, 241, 278]],
    ["BLCR", [909, 716, 667, 539], [350, 249, 250, 273]],
]


def capture_scr_data() -> list[Experiment]:
    """
    Returns a list with the timings of each experiment.
    Each index represents an experiment.
    Each experiment is a list of timings.
    Each timing is a list of [label, mean, standard_deviation, stdev_percentage].
    """
    timings = []
    for i in range(1, 25):
        timings.append([])
        with open(pp.OUTPUT_FILE.replace("$", f"{i}"), "r") as arquivo:
            csv_reader = csv.reader(arquivo)
            next(csv_reader)  # header
            for row in csv_reader:
                if row == []:
                    continue
                label, average, stdev, stdev_percentage = row
                average, stdev = float(average), float(stdev)
                stdev_percentage = float(stdev_percentage)
                timings[i - 1].append([label, average, stdev, stdev_percentage])
    return timings


def generate_scr_functions_graphic(experiment: Experiment):
    total_average_time = experiment[0][1]
    function_labels = []
    percentages = []
    percentages_stdev = []
    total_time_scr = sum([x[1] for x in experiment[2:]])

    # a/b = x%/100% -> x = a*100/b
    # print(
    #     f"Total time of SCR functions compared to total app time: {(total_time_scr / total_average_time * 100):.2f}%"
    # )
    # print(
    #     f"Highest percentage of standard deviation between SCR functions: {max([float(x[3]) for x in experiment[2:]]):.2f}%"
    # )

    plt.figure(figsize=(6, 2))

    for i, (label, average, stdev, _) in enumerate(experiment[2:]):
        if label == "SCR_Need_checkpoint":
            continue
        function_labels.append(label.strip())
        percentages.append(average / total_time_scr * 100)
        percentages_stdev.append(stdev / total_time_scr * 100)

    # order labels and percentages from highest to lowest
    ordered = sorted(
        zip(function_labels, percentages, percentages_stdev),
        key=lambda x: x[1],
        reverse=True,
    )
    function_labels = [x[0] for x in ordered]
    percentages = [x[1] for x in ordered]
    percentages_stdev = [x[2] for x in ordered]

    # for i, label in enumerate(function_labels):
    #     # add "\n" each 10 chars
    #     if len(label) > 10:
    #         function_labels[i] = label[:10] + "\n" + label[10:]

    colors = plt.cm.viridis(np.linspace(0, 1, len(function_labels)))

    # Create bar graphs
    plt.bar(
        function_labels,
        percentages,
        color=colors,
        yerr=percentages_stdev,
        ecolor="red",
        capsize=3,
    )

    # Add values above the bars
    for i in range(len(percentages)):
        plt.text(
            i,
            percentages[i] + percentages_stdev[i],
            f"{percentages[i]:.1f}",
            ha="center",
            va="bottom",
        )

    plt.ylim(top=60)

    # Add subtitles and labels
    # plt.xlabel("Funções")
    plt.xticks(rotation=20, ha="right")

    plt.subplots_adjust()

    plt.ylabel("Tempo de Execução (%)")

    # plt.title(
    #     "Comparação entre os Tempos Médios de Execução das Funções SCR\nem relação ao timing total das funções SCR"
    # )

    plt.savefig("SCR_functions.svg", format="svg")

    # Show the plot
    # plt.show()


def generate_strategy_comparison_graph(esquemas: list[Cluster_results]):
    data = [DATA[0]] + [esquemas[1], esquemas[0], esquemas[2]] + DATA[1:]

    bar_width = 0.24

    # plt.figure(figsize=(9, 3))
    plt.figure(figsize=(12, 4))

    cluster_names = ["t3 Cluster", "c5 Cluster", "c6i Cluster", "i3 Cluster"]

    # strategies = [x[0].replace(" ", "\n") for x in data]
    strategies = [x[0] for x in data]

    total_timings_clusters = [[] for _ in range(len(data[0][1]))]
    idle_timings_clusters = [[] for _ in range(len(data[0][2]))]

    total_stdev = [[] for _ in range(len(data[0][1]))]
    idle_stdev = [[] for _ in range(len(data[0][2]))]

    # colors rgb boas para graficos
    colors = [
        [0.22, 0.46, 0.70],
        [1.00, 0.50, 0.55],
        [0.17, 0.63, 0.17],
        [0.84, 0.15, 0.16],
    ]

    for i in range(len(data)):
        # strategies.append(data[i][0])
        for j in range(len(data[i][1])):
            total_timings_clusters[j].append(int(data[i][1][j]))
            idle_timings_clusters[j].append(int(data[i][2][j]))
            if i > 0 and i < 4:
                total_stdev[j].append(int(data[i][3][j]))
                idle_stdev[j].append(int(data[i][4][j]))
            else:
                total_stdev[j].append(0)
                idle_stdev[j].append(0)

    positions = []
    for i in range(len(total_timings_clusters)):
        positions.append(np.arange(len(data)) + bar_width * i)

    for i in range(len(total_timings_clusters)):
        plt.bar(
            positions[i],
            total_timings_clusters[i],
            bar_width,
            label=cluster_names[i],
            color=colors[i],
            # yerr=total_stdev[i],
            # ecolor="magenta",
            # capsize=3,
            # align="edge",
        )

    for i in range(len(idle_timings_clusters)):
        plt.bar(
            positions[i],
            idle_timings_clusters[i],
            bar_width,
            hatch="/",
            color=[x - 0.1 for x in colors[i]],
            # yerr=idle_stdev[i],
            # ecolor="gold",
            capsize=2,
            # align="edge",
        )

    # Add timing values above the bars
    for i, cluster in enumerate(total_timings_clusters):
        for j, timing in enumerate(cluster):
            plt.text(
                positions[i][j],
                timing + total_stdev[i][j] + 0.1,
                f"{timing}",
                # fontsize="x-small",
                # fontsize="7",
                fontsize="10",
                ha="center",
                va="bottom",
            )

    for i, cluster in enumerate(idle_timings_clusters):
        for j, timing in enumerate(cluster):
            if timing == 0:
                continue
            plt.text(
                positions[i][j],
                timing + idle_stdev[i][j] + 0.1,
                f"{timing}",
                # fontsize="x-small",
                # fontsize="7",
                fontsize="10",
                ha="center",
                va="bottom",
            )

    # errorbars
    dist = 0.04
    ecolor = "black"
    for i in range(len(total_timings_clusters)):
        for j in range(len(total_timings_clusters[0])):
            if j > 0 and j < 4:
                plt.errorbar(
                    positions[i][j] - dist,
                    total_timings_clusters[i][j],
                    total_stdev[i][j],
                    ecolor=ecolor,
                    capsize=2,
                )

                plt.errorbar(
                    positions[i][j] + dist,
                    idle_timings_clusters[i][j],
                    idle_stdev[i][j],
                    ecolor=ecolor,
                    capsize=2,
                )

    # plt.xlabel("Estratégias de CR")
    plt.ylabel("Tempo total (segundos)", fontsize="x-large")
    # plt.title("Tempos de execução do HEAT com instâncias AWS e duas falhas")

    plt.xticks(
        [r + 0.37 for r in range(len(data))],
        strategies,
        # fontsize="small",
        # fontsize="medium",
        fontsize="x-large",
    )
    # plt.xticks([r for r in range(len(data))], strategies, fontsize="small", )

    plt.legend(loc="upper left", fontsize="small")
    # plt.tight_layout()
    plt.savefig("strategies-comparison.svg", format="svg")
    # plt.show()


def generate_4x8_graph(schemes4, schemes8):
    bar_width = 0.35

    cluster_names = ["t3", "c5", "c6i", "i3"]

    colors = [
        [0.58, 0.40, 0.74],
        [0.54, 0.78, 0.39],
    ]
    colors_hach = [[x - 0.1 for x in cor] for cor in colors]

    plt.figure(figsize=(8.5, 5))

    plt.suptitle(
        "Comparação entre os tempos de execução do HEAT com 4 e 8 nodos (2 falhas)"
    )

    for i in range(3):
        plt.subplot(1, 3, i + 1)

        # plt.figure(figsize=(4.5, 3.5))

        plt.ylim(top=600)

        plt.title(schemes4[i][0], fontsize="medium")

        strategies = ["4 Nodos", "8 Nodos"]

        total_timings_clusters = [schemes4[i][1], schemes8[i][1]]
        idle_timings_clusters = [schemes4[i][2], schemes8[i][2]]

        for i in range(len(total_timings_clusters)):
            for j in range(len(total_timings_clusters[i])):
                total_timings_clusters[i][j] = int(total_timings_clusters[i][j])
                idle_timings_clusters[i][j] = int(idle_timings_clusters[i][j])

        positions = []
        for j in range(len(total_timings_clusters)):
            positions.append(np.arange(len(cluster_names)) + bar_width * j)

        for j in range(len(total_timings_clusters)):
            plt.bar(
                positions[j],
                total_timings_clusters[j],
                bar_width,
                label=strategies[j],
                color=[colors[j]],
                # align="edge",
            )

        for j in range(len(idle_timings_clusters)):
            plt.bar(
                positions[j],
                idle_timings_clusters[j],
                bar_width,
                hatch="/",
                color=[colors_hach[j]],
                # align="edge",
            )

        # Add timing values above the bars
        for j, cluster in enumerate(total_timings_clusters):
            for k, timing in enumerate(cluster):
                plt.text(
                    positions[j][k],
                    timing + 0.1,
                    f"{timing}",
                    fontsize="x-small",
                    ha="center",
                    va="bottom",
                )

        for j, cluster in enumerate(idle_timings_clusters):
            for k, timing in enumerate(cluster):
                if timing == 0:
                    continue
                plt.text(
                    positions[j][k],
                    timing + 0.1,
                    f"{timing}",
                    fontsize="x-small",
                    ha="center",
                    va="bottom",
                )

        plt.xlabel("Clusters")

        plt.ylabel("Tempo total (segundos)")

        plt.xticks(
            [r + bar_width * 0.48 for r in range(len(cluster_names))],
            cluster_names,
        )

        plt.legend(loc="upper left")

        plt.tight_layout()

    plt.savefig(f"4x8-SCR.svg", format="svg")
    # plt.show()


if __name__ == "__main__":
    data = capture_scr_data()

    # data[3] = exp 4: t3.2xlarge, 8 nodos, partner
    generate_scr_functions_graphic(data[3])

    strategies = ["SCR Partner", "SCR XOR", "SCR RS"]
    labels = ["tot", "idle", "tot_stdev", "idle_stdev", "n_idle"]
    partner8 = {label: [] for label in labels}
    xor8 = {label: [] for label in labels}
    rs8 = {label: [] for label in labels}
    schemes8 = [partner8, xor8, rs8]

    for i in range(3, 24, 6):
        for j in range(3):
            schemes8[j]["tot"].append(data[i + j][0][1])
            schemes8[j]["tot_stdev"].append(data[i + j][0][2])
            schemes8[j]["n_idle"].append(data[i + j][1][1])
            schemes8[j]["idle"].append(
                schemes8[j]["tot"][-1] - schemes8[j]["n_idle"][-1]
            )
            schemes8[j]["idle_stdev"].append(data[i + j][1][2])

    partner8 = ["SCR Partner", *schemes8[0].values()]
    xor8 = ["SCR XOR", *schemes8[1].values()]
    rs8 = ["SCR RS", *schemes8[2].values()]

    # remove n_idle
    partner8.pop()
    xor8.pop()
    rs8.pop()

    schemes8 = [partner8, xor8, rs8]

    generate_strategy_comparison_graph(schemes8)

    partner4 = {label: [] for label in labels}
    xor4 = {label: [] for label in labels}
    rs4 = {label: [] for label in labels}
    schemes4 = [partner4, xor4, rs4]

    for i in range(0, 24, 6):
        for j in range(3):
            schemes4[j]["tot"].append(data[i + j][0][1])
            schemes4[j]["tot_stdev"].append(data[i + j][0][2])
            schemes4[j]["n_idle"].append(data[i + j][1][1])
            schemes4[j]["idle"].append(
                schemes4[j]["tot"][-1] - schemes4[j]["n_idle"][-1]
            )
            schemes4[j]["idle_stdev"].append(data[i + j][1][2])

    partner4 = ["SCR Partner", *schemes4[0].values()]
    xor4 = ["SCR XOR", *schemes4[1].values()]
    rs4 = ["SCR RS", *schemes4[2].values()]

    # remove n_idle
    partner4.pop()
    xor4.pop()
    rs4.pop()

    schemes4 = [partner4, xor4, rs4]

    # generate_4x8_graph(schemes4, schemes8)
