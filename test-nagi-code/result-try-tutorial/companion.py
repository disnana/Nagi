"""main.nagiと同じ流れを、Pythonの通常の例外で表した比較用コードです。"""


class Quote:
    def __init__(self, amount: int):
        self.amount = amount
        self.doubled = amount * 2


def parse_amount(text: str) -> int:
    # Pythonではint(text)が成功値を直接返し、変換できなければValueErrorを送出します。
    # この関数はNagi版のResultとは異なり、戻り値の型に失敗を含めません。
    amount = int(text)
    if amount < 0:
        raise ValueError("金額は0以上で入力してください")
    return amount


def make_quote(text: str) -> Quote:
    # Pythonではここで捕捉しないValueErrorが、この関数の呼出元へ伝わります。
    return Quote(parse_amount(text))


def main() -> None:
    # Pythonではinput(prompt)が表示と入力を1回の呼出しで行います。
    text = input("金額を入力してください > ")
    try:
        quote = make_quote(text)
    except ValueError as problem:
        # Pythonではtry/exceptで例外を捕捉します。NagiのtryはErrを早期returnする別の仕組みです。
        print(problem)
        print("入力を直して再実行してください")
        return
    print(quote.amount)
    print(quote.doubled)
    print("完了")


if __name__ == "__main__":
    main()
