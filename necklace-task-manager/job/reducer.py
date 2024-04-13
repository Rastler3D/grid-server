import json
import math
import pickle


def reducer(job, results):
    grid = pickle.loads(job)

    outputs = []
    for result in results:
        result = pickle.loads(result)
        if result["valid"]:
            outputs.append(result["solution"])

    size = len(grid)
    formate_outputs = { idx: format_matrix(solution,size) for idx, solution in enumerate(outputs) }

    return json.dumps(formate_outputs)


def format_matrix(solution, size):
    length = int(math.sqrt(size))

    # Маппинг цифр на соответствующие символы
    symbol_map = {
        1: '\u2510',
        2: '\u250C',
        3: '\u2518',
        4: '\u2514',
        5: '\u2502',
        6: '\u2500',
        7: ' '
    }
    matrix = []
    for i in range(length):
        row = solution[i*length: (i+1)*length]
        matrix_row = []
        for col, cell in enumerate(row):
            matrix_row.append(symbol_map[cell])
        matrix.append(matrix_row)