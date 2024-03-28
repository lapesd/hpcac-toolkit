# um arquivo para cada experimento: 5 execuções (exp1.csv, ..., exp24.csv)
# cada arquivo tem 5 linhas (uma para cada execução):
# linhas: tempo total, tempo não idle, tempo terminar instâncias, SCR_Init,
# SCR_Have_restart, SCR_Start_restart, SCR_Route_file, SCR_Complete_restart,
# SCR_Need_checkpoint, SCR_Start_output, SCR_Complete_output, SCR_Finalize (12 colunas)
# Obs: checado, printado na ordem acima
#
# pós-processamento: calcular média e desvio padrão para cada experimento
# e escrever em um arquivo .csv (resultados/exp1.csv, ..., resultados/exp24.csv)

import csv
from pathlib import Path
import capture


INPUT_DIR = capture.OUTPUT_DIR + capture.EXP_PREFIX + "$.csv"
OUTPUT_FILE = capture.DATA_DIR + "final/" + capture.EXP_PREFIX + "$.csv"


def calcular_desvio_padrao(lista: list[float]) -> float:
    """
    Calcula o desvio padrão de uma lista de números.
    """
    n = len(lista)
    media = sum(lista) / n
    soma = 0
    for i in lista:
        soma += (i - media) ** 2
    desvio_padrao = (soma / (n - 1)) ** 0.5
    return desvio_padrao


def calcular_porcentagem_desvio_padrao(
    desvios: list[float], tempos_totais: list[float]
) -> list[float]:
    """
    Calcula a porcentagem do desvio padrão em relação ao tempo total.
    """
    # desvio/tempo_total = x%/100% -> x = 100*desvio/tempo_total

    porcentagens = []
    for desvio, tempo_total in zip(desvios, tempos_totais):
        porcentagens.append(100 * desvio / tempo_total)
    return porcentagens


def calcular_tempo_medio(lista: list[float]) -> float:
    """
    Calcula o tempo médio de uma lista de números.
    """
    return sum(lista) / len(lista)


def pegar_tempos_totais(nome_arquivo: str) -> list[float]:
    """
    Retorna uma lista com os tempos totais de cada execução.
    """
    tempos = []
    with open(nome_arquivo, "r") as arquivo:
        leitor_csv = csv.reader(arquivo)
        next(leitor_csv)  # header
        for linha in leitor_csv:
            if linha == []:
                continue
            tempos.append(float(linha[0]))
    return tempos


def pegar_tempos_nao_idle(nome_arquivo: str) -> list[float]:
    """
    Retorna uma lista com os tempos não idle de cada execução.
    """
    tempos = []
    with open(nome_arquivo, "r") as arquivo:
        leitor_csv = csv.reader(arquivo)
        next(leitor_csv)  # header
        for linha in leitor_csv:
            if linha == []:
                continue
            tempos.append(float(linha[1]))
    return tempos


def pegar_tempos_por_funcao(nome_arquivo: str) -> list[list[float]]:
    """
    Retorna uma lista com os tempos de cada função de cada execução.
    """
    tempos = [[] for _ in range(9)]
    with open(nome_arquivo, "r") as arquivo:
        leitor_csv = csv.reader(arquivo)
        next(leitor_csv)  # header
        for linha in leitor_csv:
            if linha == []:
                continue
            for i in range(2, 11):
                tempos[i - 2].append(float(linha[i]))
    return tempos


def escrever_resultados(
    nome_arquivo: str,
    media_tempos_totais: float,
    stdev_tempos_totais: float,
    porc_stdev_tempos_totais: float,
    media_tempos_nao_idle: float,
    stdev_tempos_nao_idle: float,
    porc_stdev_tempos_nao_idle: float,
    medias_tempos_por_funcao: list[float],
    stdevs_tempos_por_funcao: list[float],
    porc_stdevs_tempos_por_funcao: list[float],
) -> None:
    """
    Escreve os resultados em um arquivo .csv, com o seguinte formato:
    media_tempos_totais, stdev_tempos_totais
    media_tempos_nao_idle, stdev_tempos_nao_idle
    tempo_funcao1_medio, desvio_padrao_funcao1
    ...
    tempo_funcao10_medio, desvio_padrao_funcao10
    """
    if not Path(nome_arquivo).parent.exists():
        Path(nome_arquivo).parent.mkdir()

    header_list = capture.HEADER.split(",")

    with open(nome_arquivo, "w") as arquivo:
        escritor_csv = csv.writer(arquivo)
        escritor_csv.writerow(
            ["label", "media", "desvio_padrao", "porcentagem_desvio_padrao"]
        )
        escritor_csv.writerow(
            [
                header_list[0],
                media_tempos_totais,
                stdev_tempos_totais,
                porc_stdev_tempos_totais,
            ]
        )
        escritor_csv.writerow(
            [
                header_list[1],
                media_tempos_nao_idle,
                stdev_tempos_nao_idle,
                porc_stdev_tempos_nao_idle,
            ]
        )
        for i in range(len(medias_tempos_por_funcao)):
            escritor_csv.writerow(
                [
                    header_list[i + 2],
                    medias_tempos_por_funcao[i],
                    stdevs_tempos_por_funcao[i],
                    porc_stdevs_tempos_por_funcao[i],
                ]
            )


def main():
    """
    Executa o pós-processamento.
    """
    for i in range(1, 25):
        arquivo = INPUT_DIR.replace("$", str(i))

        tempos_totais = pegar_tempos_totais(arquivo)
        tempos_nao_idle = pegar_tempos_nao_idle(arquivo)
        tempos_por_funcao = pegar_tempos_por_funcao(arquivo)

        media_tempos_totais = calcular_tempo_medio(tempos_totais)
        stdev_tempos_totais = calcular_desvio_padrao(tempos_totais)
        porc_stdev_tempos_totais = calcular_porcentagem_desvio_padrao(
            [stdev_tempos_totais], [media_tempos_totais]
        )[0]

        media_tempos_nao_idle = calcular_tempo_medio(tempos_nao_idle)
        stdev_tempos_nao_idle = calcular_desvio_padrao(tempos_nao_idle)
        porc_stdev_tempos_nao_idle = calcular_porcentagem_desvio_padrao(
            [stdev_tempos_nao_idle], [media_tempos_nao_idle]
        )[0]

        medias_tempos_por_funcao = []
        stdevs_tempos_por_funcao = []
        for j in tempos_por_funcao:
            medias_tempos_por_funcao.append(calcular_tempo_medio(j))
            stdevs_tempos_por_funcao.append(calcular_desvio_padrao(j))
        porc_stdevs_tempos_por_funcao = calcular_porcentagem_desvio_padrao(
            stdevs_tempos_por_funcao, medias_tempos_por_funcao
        )

        escrever_resultados(
            OUTPUT_FILE.replace("$", str(i)),
            media_tempos_totais,
            stdev_tempos_totais,
            porc_stdev_tempos_totais,
            media_tempos_nao_idle,
            stdev_tempos_nao_idle,
            porc_stdev_tempos_nao_idle,
            medias_tempos_por_funcao,
            stdevs_tempos_por_funcao,
            porc_stdevs_tempos_por_funcao,
        )


if __name__ == "__main__":
    main()
