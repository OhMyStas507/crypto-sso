"""独立核对合成 v1 向量的 LP 编码、哈希及标量约减，不导入 SDK。"""
import hashlib
import json
from pathlib import Path


def lp(*parts):
    """按照协议拼接 u64 小端长度前缀和原始字节。"""
    return b"".join(len(p).to_bytes(8, "little") + p for p in parts)


def scalar(*parts):
    """SHA-512 输出作为小端整数，约减到 BLS12-381 标量域。"""
    modulus = int("73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001", 16)
    value = int.from_bytes(hashlib.sha512(lp(*parts)).digest(), "little") % modulus
    return value.to_bytes(32, "little").hex()


def main():
    vectors = json.loads(Path(__file__).with_name("vectors.json").read_text(encoding="utf-8"))
    public = json.loads(vectors["public"])
    request = json.loads(vectors["request"])
    assert hashlib.sha256(lp(b"crypto-sso/v1/enrollment-request", vectors["request"].encode())).hexdigest() == vectors["request_hash"]
    assert hashlib.sha256(lp(b"crypto-sso/v1/issuer", b"\x01", public["organization"].encode(), public["public_key"].encode(), vectors["parameters"]["rule"].encode())).hexdigest() == vectors["issuer_id"]
    assert scalar(b"crypto-sso/v1/organization-message", vectors["issuer_id"].encode()) == vectors["organization_message"]
    for key, value in request["claims"].items():
        assert scalar(b"crypto-sso/v1/claim", b"utf8-string", key.encode(), value.encode()) == vectors["claim_messages"][key]
    assert len(request["claims"]).to_bytes(32, "little").hex() == vectors["count_message"]
    print("PV-03/04：Python 独立哈希与标量编码核对通过")


if __name__ == "__main__":
    main()
