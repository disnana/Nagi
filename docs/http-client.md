# HTTP client API（SF06）

このページは未リリースの Nagi 0.2.0 開発候補の説明です。この API は公開済み Nagi 0.1.11 には含まれません。サーバー側の HTTP 機能は[HTTP server API](http-server.md)、作業範囲は[SF06 実装計画](internal/security-foundation/implementation-plan.md)を参照してください。

## API

```nagi
import std.http.client as client
import std.auth as auth
```

`std.http.client` は次の6リソースと10関数を定義します。

| リソース | 用途 |
|---|---|
| `ProtectedOperation[C, P, I]` | Grant を必要とする固定目的の操作 |
| `PublicOperation[C, I]` | 明示的に公開登録された操作 |
| `Response` | 所有された最終応答 |
| `Failure` | 分類済み失敗 |
| `FailureKind` | 失敗の分類 |
| `Outcome` | HTTP 要求の開始状態 |

| 関数 | 署名 |
|---|---|
| `execute` | `execute[C, P, I](view[ProtectedOperation[C, P, I]], auth.Grant[P], I) -> Future[Result[Response, Failure]]` |
| `execute_public` | `execute_public[C, I](view[PublicOperation[C, I]], I) -> Future[Result[Response, Failure]]` |
| `status` | `(view[Response]) -> i64` |
| `headers` | `(view[Response], view[str]) -> Result[List[view[bytes]], Error]` |
| `body` | `(view[Response]) -> view[bytes]` |
| `take_body` | `(Response) -> bytes` |
| `kind` | `(view[Failure]) -> FailureKind` |
| `auth_kind` | `(view[Failure]) -> Option[auth.FailureKind]` |
| `outcome` | `(view[Failure]) -> Outcome` |
| `message` | `(view[Failure]) -> view[str]` |

次の呼び出し形は High、保存 Low、手書き Low のコンパイラ fixture で検査されています。これは呼び出し側の型を示す例であり、単独で起動できるアプリケーション例ではありません。

```nagi
class State:
    n: i64
enum Permission:
    Allowed
async def protected_call(operation: view[client.ProtectedOperation[State, Permission, bytes]], grant: auth.Grant[Permission], input: bytes) -> Result[client.Response, client.Failure]:
    return await client.execute(operation, grant, input)
async def public_call(operation: view[client.PublicOperation[State, bytes]], input: bytes) -> Result[client.Response, client.Failure]:
    return await client.execute_public(operation, input)
```

## 登録と権限の境界

操作ハンドルは型付きで不透明であり、登録後に操作定義を変更できません。`C` はホストが保持する共有状態、`P` はnominalな権限型、`I` は所有された型付き業務入力です。Nagi から操作やクライアントを作るコンストラクターはなく、標準 API に既定の接続先・数値上限もありません。信頼された Rust ホストが起動時に固定操作を登録し、`Owner` を独立して保持して実際の終了・joinを待つ必要があります。サポート対象となる本番 profile、受理するcanonical/address形式、既定値、数値上限はまだ確定していません。テスト用の値を本番設定として使わないでください。

保護操作では `execute` が実際の `Grant[P]` を消費します。ホストのレビュー済み adapter は、その Grant から得た subject/target を実際の要求へ結び付けます。呼び出し側から任意の URL、method/path の組み合わせ、raw header map、Cookie や cookie jar を渡す API はありません。ヘッダーは登録時の固定 `Accept` / `Content-Type` 宣言と、保護操作に結び付いた任意の Bearer 認証情報に限られます。公開操作は別の型で登録され、Grant や認証情報を受け取りません。`ProtectedOperation` を `execute_public` に渡して認可を迂回することは、型検査で拒否します。

この型境界は、信頼された Rust adapter の実装や相手サービスの認証を検証しません。adapter は実際の subject/target を使い、相手サービスも独立して要求を認証・認可する必要があります。

操作ハンドル、`Response`、`Failure` は非Copy・非Cloneの所有値です。読むときは `view`、所有ローカルを別のローカルへ移すときは既存の明示 `move` を使います。`shared` の所有者を増やす操作はpayloadの独立コピーとは異なり、共有された `Response` の本文を `take_body` で取り出すことはできません。`FailureKind` と `Outcome` はCopy値です。`Grant` はハンドルと異なる短命な認可proofであり、別Taskや入力wrapperへ逃がせません。

ハンドルをDropしても、ホストが保持するOwnerの終了確認にはなりません。ホストは新規受理を閉じ、取消を要求した処理の実際の終了・joinを待ちます。実行FutureのDropは終了確認と別です。取消は送信済みの要求や相手側の業務処理を巻き戻しません。予期しないpanicやtask故障を通常の業務`Failure`へ一律に変換せず、既存の故障・cleanup契約を維持します。任意Rustコードの強制停止やOOMからの普遍的回復は保証しません。

## 応答と失敗

完了した応答は、すべての実際の子タスク処理と join、および最終公開判定が終わってから、本文を含む所有値 `Response` として返ります。応答はソケットや実行中ジョブを保持しません。`status` は数値 status を返します。HTTP 403 や 302 など、通信として受け取った通常の status は `Err(Failure)` ではなく `Ok(Response)` です。302を追跡せず、HTTP 要求を自動再送しません。公開するのは初期応答ヘッダーです。trailer は上限内で検証しますが、公開 API には含まれません。

`headers` は大文字・小文字を区別しない名前で検索し、重複を保った同名の値をすべて返します。値のbytesはコピーせず `Response` から借用し、検索名からは借用しません。返却するList自体には確保が発生します。ヘッダー名の検証に失敗した場合は accessor の `Error` です。返された view を使っている間は `Response` をmoveできません。`body` は本文を借用し、`take_body` は `Response` を消費して元のVecのバッファをpayloadの追加コピーなしで移します。通信全体が無確保・無コストという保証ではありません。相手のヘッダー・本文は信頼されていないデータであり、受信成功が業務上の検証成功を意味するわけではありません。

`FailureKind` の14分類は `INVALID_INPUT`, `SECURITY`, `DESTINATION_DENIED`, `CAPACITY`, `CLOSING`, `DEADLINE`, `DNS`, `TCP`, `TLS`, `PROTOCOL`, `RESPONSE_LIMIT`, `UNSUPPORTED_ENCODING`, `UNAVAILABLE`, `INTERNAL` です。protected admissionの認証・認可失敗は `SECURITY` に分類され、`auth_kind` が元の `auth.FailureKind` を保持します。それ以外は `None` です。`message` は秘密を含まない固定の説明です。accessor の `Error` と要求の `Failure` は別です。

`Outcome` は `REQUEST_NOT_STARTED` または `UNKNOWN` です。HTTP の同期送信開始前に失敗した場合は `REQUEST_NOT_STARTED` です。送信開始後の失敗は、相手が要求を受け取ったか確認できないため `UNKNOWN` です。開始前でもDNSやTCP通信が発生している場合があります。`UNKNOWN` は再送の許可を意味しません。

## 接続動作と現在の検証範囲

DNS応答のA/AAAA候補をすべて検査します。危険な候補またはいずれかのfamilyのlookup errorがあれば拒否します。許可された候補は元の期限内で有限個を逐次試し、再試行するのは対象となるTCP接続失敗だけです。TTL 0 はその一回の解決操作だけに使い、cacheには保存しません。TLSの通常の証明書検証を行い、応答本文は上限付きです。圧縮応答は0.2.0では未対応で、0.2.1での対応が計画されています（#124）。

| 検証 | 結果と範囲 |
|---|---|
| High・保存 Low・手書き Low の3形式、2 fixture groups | PASS。呼び出し/API projection の検証であり、一般の本番ホスト構成を示しません |
| 正のTLS native fixture | PASS。ただし送信先の許可は `cfg(test)` 専用 seam を使います。本番の正の接続profileを証明しません |
| 本番のloopback拒否 | PASS。A検索はloopbackを返し、AAAA検索は正常なNoDataとして完了します。TCP接続前の拒否を実タスク終了後に確認します |
| 4 OS の公開API接続CI | 保留中。過去のprivate-head CI結果はこの候補の証拠ではありません |

この候補はリリース済み機能でも、4 OS の本番接続受入れ済み機能でもありません。
