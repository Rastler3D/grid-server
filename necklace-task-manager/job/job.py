import json
import pickle


def extract_job(json_data):
    grid = json.loads(json_data)

    if type(grid) is not list or not all(isinstance(row, list) for row in grid) or len(set(map(len, grid))) != 1:
        raise ValueError("Job must be two dimensional array of char")

    output = []

    for row in grid:
        if all(cell in ["a", "b", "0"] for cell in row):
            output.extend(row)
        else:
            raise ValueError("Char must be one of 'a', 'b', '0' ")

    return pickle.dumps(output)