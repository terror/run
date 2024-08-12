from functools import lru_cache

@lru_cache
def fib(n):
  return n if n < 2 else fib(n - 1) + fib(n - 2)

print(sum(filter(lambda x: x % 2 == 0, map(lambda x: fib(x), range(34)))))
