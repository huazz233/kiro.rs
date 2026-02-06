#!/usr/bin/env python3
"""
将 Kiro IDE 凭证格式转换为 kiro-rs credentials.json 格式
"""

import json

# 硬编码路径 - 根据需要修改
INPUT_FILE = r"D:\code\pro\codeProject\github_star\kiro.rs\kiro_accounts.json"
OUTPUT_FILE = r"D:\code\pro\codeProject\github_star\kiro.rs\config\credentials.json"


def convert():
    # 读取 Kiro IDE 格式
    with open(INPUT_FILE, "r", encoding="utf-8") as f:
        data = json.load(f)

    # 转换为 kiro-rs 格式
    credentials = []
    for idx, account in enumerate(data.get("accounts", [])):
        # 不再过滤 enabled: false 的账号，全部转换
        cred = account.get("credentials", {})
        item = {
            "accessToken": cred.get("accessToken"),
            "refreshToken": cred.get("refreshToken"),
            "expiresAt": cred.get("expiresAt"),
            "authMethod": cred.get("authMethod", "idc"),
            "priority": idx,
        }

        # 可选字段
        if cred.get("region"):
            item["region"] = cred["region"]
        if cred.get("clientId"):
            item["clientId"] = cred["clientId"]
        if cred.get("clientSecret"):
            item["clientSecret"] = cred["clientSecret"]
        if cred.get("profileArn"):
            item["profileArn"] = cred["profileArn"]

        credentials.append(item)

    # 写入输出文件
    with open(OUTPUT_FILE, "w", encoding="utf-8") as f:
        json.dump(credentials, f, indent=2, ensure_ascii=False)

    print(f"转换完成: {len(credentials)} 个凭据")
    print(f"输出文件: {OUTPUT_FILE}")


if __name__ == "__main__":
    convert()
