import matplotlib.pyplot as plt
import numpy as np
import csv
import post_processing as pp
import typing as tp

# experimentos:
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

Experimento = tp.Tuple[tp.Tuple[str, float, float]]

Nome_da_estrategia = tp.NewType("Nome_da_estrategia", str)
Tempos_totais = tp.List[int]
Tempos_ociosos = tp.List[int | str]  # lista vazia -> str (PEP 484)
Tempos_totais_stdev = tp.List[int]
Tempos_ociosos_stdev = tp.List[int]

# Indices: 0 - t3.2xlarge, 1 - c5.2xlarge, 2 - c6i.2xlarge, 3 - i3.2xlarge
Cluster_results = tp.Tuple[
    Nome_da_estrategia,
    Tempos_totais,
    Tempos_ociosos,
    Tempos_totais_stdev,
    Tempos_ociosos_stdev,
]


DATA: list[Cluster_results] = [
    ["Sem Faltas", [181, 89, 81, 90], [0, 0, 0, 0]],
    ["ULFM", [537, 385, 341, 380], [327, 263, 241, 278]],
    ["BLCR", [909, 716, 667, 539], [350, 249, 250, 273]],
]


def pegar_dados_scr() -> list[Experimento]:
    """
    Retorna uma lista com os tempos processados de cada experimento.
    Cada índice da lista representa um experimento.
    Cada experimento é uma lista de tempos.
    Cada tempo é uma lista de [rotulo, media, desvio_padrao, porcentagem_desvio_padrao].
    """
    tempos = []
    for i in range(1, 25):
        tempos.append([])
        with open(pp.OUTPUT_FILE.replace("$", f"{i}"), "r") as arquivo:
            leitor_csv = csv.reader(arquivo)
            next(leitor_csv)  # header
            for linha in leitor_csv:
                if linha == []:
                    continue
                rotulo, media, desvio_padrao, porcentagem_stdev = linha
                media, desvio_padrao = float(media), float(desvio_padrao)
                porcentagem_stdev = float(porcentagem_stdev)
                tempos[i - 1].append([rotulo, media, desvio_padrao, porcentagem_stdev])
    return tempos


def gerar_grafico_funcoes_SCR(experimento: Experimento):
    tempo_total_medio = experimento[0][1]
    rotulos_funcoes = []
    porcentagens = []
    porcentagens_stdev = []
    tempo_total_scr = sum([x[1] for x in experimento[2:]])

    # a/b = x%/100% -> x = a*100/b
    # print(
    #     f"Tempo total das funções SCR em comparação com o tempo total da aplicação: {(tempo_total_scr / tempo_total_medio * 100):.2f}%"
    # )
    # print(
    #     f"Maior porcentagem de desvio padrão entre as funções SCR: {max([float(x[3]) for x in experimento[2:]]):.2f}%"
    # )

    plt.figure(figsize=(6, 2))

    for i, (rotulo, media, stdev, _) in enumerate(experimento[2:]):
        if rotulo == "SCR_Need_checkpoint":
            continue
        rotulos_funcoes.append(rotulo.strip())
        porcentagens.append(media / tempo_total_scr * 100)
        porcentagens_stdev.append(stdev / tempo_total_scr * 100)

    # ordenar rotulos e porcentagens do maior para o menor
    ordenados = sorted(
        zip(rotulos_funcoes, porcentagens, porcentagens_stdev),
        key=lambda x: x[1],
        reverse=True,
    )
    rotulos_funcoes = [x[0] for x in ordenados]
    porcentagens = [x[1] for x in ordenados]
    porcentagens_stdev = [x[2] for x in ordenados]

    # for i, rotulo in enumerate(rotulos_funcoes):
    #     # adicionar "\n" a cada 10 caracteres
    #     if len(rotulo) > 10:
    #         rotulos_funcoes[i] = rotulo[:10] + "\n" + rotulo[10:]

    cores = plt.cm.viridis(np.linspace(0, 1, len(rotulos_funcoes)))

    # Criar os gráficos de barras
    plt.bar(
        rotulos_funcoes,
        porcentagens,
        color=cores,
        yerr=porcentagens_stdev,
        ecolor="red",
        capsize=3,
    )

    # Adicionar os valores acima das barras
    for i in range(len(porcentagens)):
        plt.text(
            i,
            porcentagens[i] + porcentagens_stdev[i],
            f"{porcentagens[i]:.1f}",
            ha="center",
            va="bottom",
        )

    plt.ylim(top=60)
    # Adicionar legenda e rótulos
    # plt.xlabel("Funções")
    plt.xticks(rotation=20, ha="right")

    plt.subplots_adjust()

    plt.ylabel("Tempo de Execução (%)")

    # plt.title(
    #     "Comparação entre os Tempos Médios de Execução das Funções SCR\nem relação ao tempo total das funções SCR"
    # )

    plt.savefig("funcoes_SCR.svg", format="svg")

    # Exibir o gráfico
    # plt.show()


def gerar_grafico_comparacao_estrategias(esquemas: list[Cluster_results]):
    data = [DATA[0]] + [esquemas[1], esquemas[0], esquemas[2]] + DATA[1:]

    largura_barra = 0.24

    # plt.figure(figsize=(9, 3))
    plt.figure(figsize=(12, 4))

    nomes_clusters = ["t3 Cluster", "c5 Cluster", "c6i Cluster", "i3 Cluster"]

    # estrategias = [x[0].replace(" ", "\n") for x in data]
    estrategias = [x[0] for x in data]

    clusters_total = [[] for _ in range(len(data[0][1]))]
    clusters_ocioso = [[] for _ in range(len(data[0][2]))]

    total_stdev = [[] for _ in range(len(data[0][1]))]
    ocioso_stdev = [[] for _ in range(len(data[0][2]))]

    # cores rgb boas para graficos
    cores = [
        [0.22, 0.46, 0.70],
        [1.00, 0.50, 0.55],
        [0.17, 0.63, 0.17],
        [0.84, 0.15, 0.16],
    ]

    for i in range(len(data)):
        # estrategias.append(data[i][0])
        for j in range(len(data[i][1])):
            clusters_total[j].append(int(data[i][1][j]))
            clusters_ocioso[j].append(int(data[i][2][j]))
            if i > 0 and i < 4:
                total_stdev[j].append(int(data[i][3][j]))
                ocioso_stdev[j].append(int(data[i][4][j]))
            else:
                total_stdev[j].append(0)
                ocioso_stdev[j].append(0)

    posicoes = []
    for i in range(len(clusters_total)):
        posicoes.append(np.arange(len(data)) + largura_barra * i)

    for i in range(len(clusters_total)):
        plt.bar(
            posicoes[i],
            clusters_total[i],
            largura_barra,
            label=nomes_clusters[i],
            color=cores[i],
            # yerr=total_stdev[i],
            # ecolor="magenta",
            # capsize=3,
            # align="edge",
        )

    for i in range(len(clusters_ocioso)):
        plt.bar(
            posicoes[i],
            clusters_ocioso[i],
            largura_barra,
            hatch="/",
            color=[x - 0.1 for x in cores[i]],
            # yerr=ocioso_stdev[i],
            # ecolor="gold",
            capsize=2,
            # align="edge",
        )

    # Adicionar os textos dos tempos acima das barras
    for i, cluster in enumerate(clusters_total):
        for j, tempo in enumerate(cluster):
            plt.text(
                posicoes[i][j],
                tempo + total_stdev[i][j] + 0.1,
                f"{tempo}",
                # fontsize="x-small",
                # fontsize="7",
                fontsize="10",
                ha="center",
                va="bottom",
            )

    for i, cluster in enumerate(clusters_ocioso):
        for j, tempo in enumerate(cluster):
            if tempo == 0:
                continue
            plt.text(
                posicoes[i][j],
                tempo + ocioso_stdev[i][j] + 0.1,
                f"{tempo}",
                # fontsize="x-small",
                # fontsize="7",
                fontsize="10",
                ha="center",
                va="bottom",
            )

    # errorbars
    dist = 0.04
    ecolor = "black"
    for i in range(len(clusters_total)):
        for j in range(len(clusters_total[0])):
            if j > 0 and j < 4:
                plt.errorbar(
                    posicoes[i][j] - dist,
                    clusters_total[i][j],
                    total_stdev[i][j],
                    ecolor=ecolor,
                    capsize=2,
                )

                plt.errorbar(
                    posicoes[i][j] + dist,
                    clusters_ocioso[i][j],
                    ocioso_stdev[i][j],
                    ecolor=ecolor,
                    capsize=2,
                )

    # plt.xlabel("Estratégias de CR")
    plt.ylabel("Tempo total (segundos)", fontsize="x-large")
    # plt.title("Tempos de execução do HEAT com instâncias AWS e duas falhas")
    # plt.xticks([r + espacamento_grupo for r in range(len(data))], estrategias, fontsize="small", ha="left")
    plt.xticks(
        [r + 0.37 for r in range(len(data))],
        estrategias,
        # fontsize="small",
        # fontsize="medium",
        fontsize="x-large",
    )
    # plt.xticks([r for r in range(len(data))], estrategias, fontsize="small", )

    plt.legend(loc="upper left", fontsize="small")
    # plt.tight_layout()
    plt.savefig("comparacao-estrategias.svg", format="svg")
    # plt.show()


def gerar_grafico_4_8_nodos(esquemas4, esquemas8):
    largura_barra = 0.35

    nomes_clusters = ["t3", "c5", "c6i", "i3"]

    cores = [
        [0.58, 0.40, 0.74],
        [0.54, 0.78, 0.39],
    ]
    cores_hach = [[x - 0.1 for x in cor] for cor in cores]

    plt.figure(figsize=(8.5, 5))

    plt.suptitle(
        "Comparação entre os tempos de execução do HEAT com 4 e 8 nodos (2 falhas)"
    )

    for i in range(3):
        plt.subplot(1, 3, i + 1)

        # plt.figure(figsize=(4.5, 3.5))

        plt.ylim(top=600)

        plt.title(esquemas4[i][0], fontsize="medium")

        estrategias = ["4 Nodos", "8 Nodos"]

        clusters_total = [esquemas4[i][1], esquemas8[i][1]]
        clusters_ocioso = [esquemas4[i][2], esquemas8[i][2]]

        for i in range(len(clusters_total)):
            for j in range(len(clusters_total[i])):
                clusters_total[i][j] = int(clusters_total[i][j])
                clusters_ocioso[i][j] = int(clusters_ocioso[i][j])

        posicoes = []
        for j in range(len(clusters_total)):
            posicoes.append(np.arange(len(nomes_clusters)) + largura_barra * j)

        for j in range(len(clusters_total)):
            plt.bar(
                posicoes[j],
                clusters_total[j],
                largura_barra,
                label=estrategias[j],
                color=[cores[j]],
                # align="edge",
            )

        for j in range(len(clusters_ocioso)):
            plt.bar(
                posicoes[j],
                clusters_ocioso[j],
                largura_barra,
                hatch="/",
                color=[cores_hach[j]],
                # align="edge",
            )

        # Adicionar os textos dos tempos acima das barras
        for j, cluster in enumerate(clusters_total):
            for k, tempo in enumerate(cluster):
                plt.text(
                    posicoes[j][k],
                    tempo + 0.1,
                    f"{tempo}",
                    fontsize="x-small",
                    ha="center",
                    va="bottom",
                )

        for j, cluster in enumerate(clusters_ocioso):
            for k, tempo in enumerate(cluster):
                if tempo == 0:
                    continue
                plt.text(
                    posicoes[j][k],
                    tempo + 0.1,
                    f"{tempo}",
                    fontsize="x-small",
                    ha="center",
                    va="bottom",
                )

        plt.xlabel("Clusters")

        plt.ylabel("Tempo total (segundos)")

        plt.xticks(
            [r + largura_barra * 0.48 for r in range(len(nomes_clusters))],
            nomes_clusters,
        )

        plt.legend(loc="upper left")

        plt.tight_layout()

    plt.savefig(f"4x8-SCR.svg", format="svg")
    # plt.show()


if __name__ == "__main__":
    dados = pegar_dados_scr()

    # dados[3] = exp 4: t3.2xlarge, 8 nodos, partner
    gerar_grafico_funcoes_SCR(dados[3])

    # print("dados[3]", dados[3])

    estrategias = ["SCR Partner", "SCR XOR", "SCR RS"]
    labels = ["tot", "idle", "tot_stdev", "idle_stdev", "n_idle"]
    partner8 = {label: [] for label in labels}
    xor8 = {label: [] for label in labels}
    rs8 = {label: [] for label in labels}
    esquemas8 = [partner8, xor8, rs8]

    for i in range(3, 24, 6):
        for j in range(3):
            esquemas8[j]["tot"].append(dados[i + j][0][1])
            esquemas8[j]["tot_stdev"].append(dados[i + j][0][2])
            esquemas8[j]["n_idle"].append(dados[i + j][1][1])
            esquemas8[j]["idle"].append(
                esquemas8[j]["tot"][-1] - esquemas8[j]["n_idle"][-1]
            )
            esquemas8[j]["idle_stdev"].append(dados[i + j][1][2])

    partner8 = ["SCR Partner", *esquemas8[0].values()]
    xor8 = ["SCR XOR", *esquemas8[1].values()]
    rs8 = ["SCR RS", *esquemas8[2].values()]

    # remove n_idle
    partner8.pop()
    xor8.pop()
    rs8.pop()

    esquemas8 = [partner8, xor8, rs8]

    gerar_grafico_comparacao_estrategias(esquemas8)

    partner4 = {label: [] for label in labels}
    xor4 = {label: [] for label in labels}
    rs4 = {label: [] for label in labels}
    esquemas4 = [partner4, xor4, rs4]

    for i in range(0, 24, 6):
        for j in range(3):
            esquemas4[j]["tot"].append(dados[i + j][0][1])
            esquemas4[j]["tot_stdev"].append(dados[i + j][0][2])
            esquemas4[j]["n_idle"].append(dados[i + j][1][1])
            esquemas4[j]["idle"].append(
                esquemas4[j]["tot"][-1] - esquemas4[j]["n_idle"][-1]
            )
            esquemas4[j]["idle_stdev"].append(dados[i + j][1][2])

    partner4 = ["SCR Partner", *esquemas4[0].values()]
    xor4 = ["SCR XOR", *esquemas4[1].values()]
    rs4 = ["SCR RS", *esquemas4[2].values()]

    # remove n_idle
    partner4.pop()
    xor4.pop()
    rs4.pop()

    esquemas4 = [partner4, xor4, rs4]

    # gerar_grafico_4_8_nodos(esquemas4, esquemas8)
