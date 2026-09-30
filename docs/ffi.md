# FFIとABIの方針

0.1にはLowのFFI構文はありません。SQLiteへのFFIは既存rusqlite/libsqlite3-sysが担当します。ユーザーのLowから任意C関数を呼ぶ経路は未実装です。

High/Lowの同じprimitiveとclassは同じRust型へ生成します。同一ビルド内の呼び出しでserializationは不要です。ただし、Rust struct layout、String、Vec、ResultをそのままC ABIへ出す方針ではありません。

将来のC ABIでは、固定幅primitive、pointer+lengthのslice、固定layout record、tag+payloadのerror表現、明示的allocator/deallocatorを定義します。所有権を受け渡すboundaryには、解放者とライブラリ寿命を指定する必要があります。

Rustは現在、`@rust`と`extern def/fn`で同じ生成crate内の関数を呼べます。`--rust`で通常のRust moduleを、`--rust-dep`でCargo依存を指定します。詳細は [modules-and-rust.md](modules-and-rust.md) を参照してください。これはC ABIを経由しない接続です。Python extension等に安定したABIを提供する前には、borrowed buffer、async task、errorの寿命とlayoutを確定する必要があります。
