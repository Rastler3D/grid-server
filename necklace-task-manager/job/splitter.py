import pickle
from itertools import product


def splitter(job):
    grid = pickle.loads(job)
    count, combinations = generate_solutions(grid)
    return count, (pickle.dumps(combination) for combination in combinations)


# Функция для генерации всех возможных решений
def generate_solutions(grid):
    size = len(grid)
    possible_values = range(1, 8)  # Возможные значения от 1 до 7
    count = pow(7, size)
    # Генерируем все возможные комбинации для клеток
    return count, product(possible_values, repeat=size)

