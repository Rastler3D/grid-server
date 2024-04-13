import math
import pickle
import job


class Job(job.Job):
    def execute_job(self, x: bytes, y: bytes) -> bytes:
        return execute_job(x, y)


def execute_job(job_data, task_data):
    grid = pickle.loads(job_data)
    solution = pickle.loads(task_data)

    valid = is_valid_solution(solution, grid)

    return pickle.dumps(dict(solution=solution, valid=valid))


def is_valid_solution(solution, grid):
    size = len(grid)
    length = int(math.sqrt(size))

    for i in range(len(solution)):
        # Проверяем условие замкнутости (у каждой клетки ровно 2 соседа)
        try:
            if solution[i] == 1:
                if solution[i - 1] not in [2, 4, 6]:
                    return False
                if solution[i + length] not in [3, 4, 5]:
                    return False
            elif solution[i] == 2:
                if solution[i + 1] not in [1, 3, 6]:
                    return False
                if solution[i + length] not in [3, 4, 5]:
                    return False
            elif solution[i] == 3:
                if solution[i - 1] not in [2, 4, 6]:
                    return False
                if solution[i - length] not in [1, 2, 5]:
                    return False
            elif solution[i] == 4:
                if solution[i + 1] not in [1, 3, 6]:
                    return False
                if solution[i - length] not in [1, 2, 5]:
                    return False
            elif solution[i] == 5:
                if solution[i + length] not in [3, 4]:
                    return False
                if solution[i - length] not in [1, 2]:
                    return False
            elif solution[i] == 6:
                if solution[i + 1] not in [1, 3]:
                    return False
                if solution[i - 1] not in [2, 4]:
                    return False
        except IndexError:
            return False

        # Проверяем на соответствие границам
        if i < length:  # Если в первой строке
            if solution[i] in [3, 4, 5]:
                return False
        if i >= size - length:  # Если в последней строке
            if solution[i] in [1, 2, 5]:
                return False
        if i % length == 0:  # Если в левом ряду
            if solution[i] in [1, 3, 6]:
                return False
        if i % length == length - 1:  # Если в правом ряду
            if solution[i] in [2, 4, 6]:
                return False

        # Проверяем на соответствие таблице точек
        if grid[i] == 'a':
            if solution[i] not in [1, 2, 3, 4]:
                return False
        if grid[i] == 'b':
            if solution[i] not in [5, 6]:
                return False
    return True
