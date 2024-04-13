from typing import TypeVar, Generic, Union, Optional, Protocol, Tuple, List, Any, Self
from enum import Flag, Enum, auto
from dataclasses import dataclass
from abc import abstractmethod
import weakref

from .types import Result, Ok, Err, Some



class Job(Protocol):

    @abstractmethod
    def execute_job(self, x: bytes, y: bytes) -> bytes:
        """
        Raises: `job.types.Err(job.imports.str)`
        """
        raise NotImplementedError

