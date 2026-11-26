# Single-line comment with keywords like def, return, class, and int

from dataclasses import dataclass
from typing import Optional


@dataclass
class SampleConfig:
    count: int
    ratio: float
    active: bool

    def summary(self) -> str:
        return f"count={self.count}, ratio={self.ratio}, active={self.active}"


def classify(n: int) -> str:
    if n < 0:
        return "negative"
    elif n == 0:
        return "zero"
    else:
        return "positive"


async def fetch_value(flag: Optional[bool] = None) -> int:
    if flag is None:
        return 0
    return 1 if flag is True else -1


def main() -> None:
    hex_val: int = 0xFF
    dec_val: int = 12345
    float_val: float = 3.14
    single_quoted: str = 'hello \'python\' world'
    double_quoted: str = "escaped \"quote\" and newline\n"
    items: list[int] = [1, 2, 3, 4, 5]
    mapping: dict[str, bool] = {"ok": True, "err": False}

    total: int = 0
    for i in range(10):
        if i == 7:
            break
        if i % 2 == 0:
            continue
        total += i

    while total > 20:
        total -= 1

    try:
        label = classify(dec_val)
    except Exception as exc:
        raise RuntimeError("classification failed") from exc
    finally:
        pass

    cfg = SampleConfig(count=total, ratio=float_val, active=mapping["ok"])
    print(f"{label} | hex={hex_val} | {cfg.summary()} | {single_quoted} | {double_quoted} | {items}")

"""
test multi line comment
"""

'''
test multi line comment
'''

if __name__ == "__main__":
    main()
