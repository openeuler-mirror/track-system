/*
 * Copyright(c) 2024-2026 China Telecom Cloud Technologies Co., Ltd. All rights
 * reserved. track-system is licensed under Mulan PSL v2. You can use this software
 * according to the terms and conditions of the Mulan PSL V2. You may obtain a
 * copy of Mulan PSL v2 at: http://license.coscl.org.cn/MulanPSL2.
 * THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY
 * KIND, EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT,
 * MERCHANTABILITY OR FITNESS FOR A PARTICULAR PURPOSE.  See the Mulan PSL v2 for
 * more details.
 */

//! AES-256-GCM secret loading helpers.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use std::{env, fs, path::Path};

const AES_GCM_NONCE_LEN: usize = 12;

/// Reads a base64(nonce[12] + AES-256-GCM ciphertext) secret from environment.
pub fn decrypt_secret_from_env(encrypted_var: &str, key_file_var: &str) -> Result<String> {
    let encrypted = required_env(encrypted_var)?;
    let key_file = required_env(key_file_var)?;
    decrypt_secret(&encrypted, Path::new(&key_file))
}

pub fn decrypt_secret(encrypted: &str, key_file: &Path) -> Result<String> {
    let key = read_key(key_file)?;
    let payload = general_purpose::STANDARD
        .decode(encrypted.trim())
        .context("解析加密密钥失败：密文不是有效 base64")?;
    if payload.len() <= AES_GCM_NONCE_LEN {
        anyhow::bail!("解析加密密钥失败：密文长度不足");
    }

    let (nonce_bytes, ciphertext) = payload.split_at(AES_GCM_NONCE_LEN);
    let cipher = Aes256Gcm::new_from_slice(&key).context("初始化密钥解密器失败")?;
    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
        .map_err(|_| anyhow::anyhow!("解密密钥失败"))?;
    String::from_utf8(plaintext).context("解密密钥失败：明文不是有效 UTF-8")
}

fn required_env(name: &str) -> Result<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .with_context(|| format!("{name} 未配置"))
}

fn read_key(key_file: &Path) -> Result<[u8; 32]> {
    let raw = fs::read_to_string(key_file)
        .with_context(|| format!("读取密钥文件失败: {}", key_file.display()))?;
    let key_text = raw.trim().strip_prefix("base64:").unwrap_or(raw.trim());
    let decoded = general_purpose::STANDARD
        .decode(key_text)
        .context("解析密钥文件失败：内容不是有效 base64")?;
    if decoded.len() != 32 {
        anyhow::bail!("解析密钥文件失败：AES-256-GCM 密钥必须是 32 字节");
    }

    let mut key = [0_u8; 32];
    key.copy_from_slice(&decoded);
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes_gcm::{aead::OsRng, AeadCore};
    use tempfile::tempdir;

    #[test]
    fn decrypts_aes_gcm_secret() {
        let dir = tempdir().unwrap();
        let key_path = dir.path().join("secret.key");
        let key = [7_u8; 32];
        fs::write(
            &key_path,
            format!("base64:{}", general_purpose::STANDARD.encode(key)),
        )
        .unwrap();

        let cipher = Aes256Gcm::new_from_slice(&key).unwrap();
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = cipher.encrypt(&nonce, b"test-secret".as_slice()).unwrap();
        let mut payload = nonce.to_vec();
        payload.extend(ciphertext);

        let encrypted = general_purpose::STANDARD.encode(payload);
        assert_eq!(
            decrypt_secret(&encrypted, &key_path).unwrap(),
            "test-secret"
        );
    }
}
