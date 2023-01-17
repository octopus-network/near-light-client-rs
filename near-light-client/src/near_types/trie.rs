pub mod nibble_slice;

use self::nibble_slice::NibbleSlice;
use super::{hash::sha256, CryptoHash};
use crate::StateProofVerificationError;
use alloc::vec::Vec;
use borsh::maybestd::{
    io::{Cursor, Error, ErrorKind, Read},
    vec,
};
use byteorder::{LittleEndian, ReadBytesExt};

#[derive(Debug, Eq, PartialEq)]
pub struct RawTrieNodeWithSize {
    node: RawTrieNode,
    memory_usage: u64,
}

#[derive(Debug, Eq, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum RawTrieNode {
    Leaf(Vec<u8>, u32, CryptoHash),
    Branch([Option<CryptoHash>; 16], Option<(u32, CryptoHash)>),
    Extension(Vec<u8>, CryptoHash),
}

const LEAF_NODE: u8 = 0;
const BRANCH_NODE_NO_VALUE: u8 = 1;
const BRANCH_NODE_WITH_VALUE: u8 = 2;
const EXTENSION_NODE: u8 = 3;

fn decode_children(cursor: &mut Cursor<&[u8]>) -> Result<[Option<CryptoHash>; 16], Error> {
    let mut children: [Option<CryptoHash>; 16] = Default::default();
    let bitmap = cursor.read_u16::<LittleEndian>()?;
    let mut pos = 1;
    for child in &mut children {
        if bitmap & pos != 0 {
            let mut arr = [0; 32];
            cursor.read_exact(&mut arr)?;
            *child = Some(CryptoHash::try_from(&arr[..]).unwrap());
        }
        pos <<= 1;
    }
    Ok(children)
}

impl RawTrieNode {
    fn encode_into(&self, out: &mut Vec<u8>) {
        // size in state_parts = size + 8 for RawTrieNodeWithSize + 8 for borsh vector length
        match &self {
            // size <= 1 + 4 + 4 + 32 + key_length + value_length
            RawTrieNode::Leaf(key, value_length, value_hash) => {
                out.push(LEAF_NODE);
                out.extend((key.len() as u32).to_le_bytes());
                out.extend(key);
                out.extend((*value_length as u32).to_le_bytes());
                out.extend(value_hash.as_bytes());
            }
            // size <= 1 + 4 + 32 + value_length + 2 + 32 * num_children
            RawTrieNode::Branch(children, value) => {
                if let Some((value_length, value_hash)) = value {
                    out.push(BRANCH_NODE_WITH_VALUE);
                    out.extend((*value_length as u32).to_le_bytes());
                    out.extend(value_hash.as_bytes());
                } else {
                    out.push(BRANCH_NODE_NO_VALUE);
                }
                let mut bitmap: u16 = 0;
                let mut pos: u16 = 1;
                for child in children.iter() {
                    if child.is_some() {
                        bitmap |= pos
                    }
                    pos <<= 1;
                }
                out.extend(bitmap.to_le_bytes());
                for child in children.iter() {
                    if let Some(hash) = child {
                        out.extend(hash.as_bytes());
                    }
                }
            }
            // size <= 1 + 4 + key_length + 32
            RawTrieNode::Extension(key, child) => {
                out.push(EXTENSION_NODE);
                out.extend((key.len() as u32).to_le_bytes());
                out.extend(key);
                out.extend(child.as_bytes());
            }
        }
    }

    fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let mut cursor = Cursor::new(bytes);
        match cursor.read_u8()? {
            LEAF_NODE => {
                let key_length = cursor.read_u32::<LittleEndian>()?;
                let mut key = vec![0; key_length as usize];
                cursor.read_exact(&mut key)?;
                let value_length = cursor.read_u32::<LittleEndian>()?;
                let mut arr = [0; 32];
                cursor.read_exact(&mut arr)?;
                let value_hash = CryptoHash(arr);
                Ok(RawTrieNode::Leaf(key, value_length, value_hash))
            }
            BRANCH_NODE_NO_VALUE => {
                let children = decode_children(&mut cursor)?;
                Ok(RawTrieNode::Branch(children, None))
            }
            BRANCH_NODE_WITH_VALUE => {
                let value_length = cursor.read_u32::<LittleEndian>()?;
                let mut arr = [0; 32];
                cursor.read_exact(&mut arr)?;
                let value_hash = CryptoHash(arr);
                let children = decode_children(&mut cursor)?;
                Ok(RawTrieNode::Branch(
                    children,
                    Some((value_length, value_hash)),
                ))
            }
            EXTENSION_NODE => {
                let key_length = cursor.read_u32::<LittleEndian>()?;
                let mut key = vec![0; key_length as usize];
                cursor.read_exact(&mut key)?;
                let mut child = [0; 32];
                cursor.read_exact(&mut child)?;
                Ok(RawTrieNode::Extension(key, CryptoHash(child)))
            }
            _ => Err(Error::new(
                borsh::maybestd::io::ErrorKind::Other,
                "Wrong type",
            )),
        }
    }
}

impl RawTrieNodeWithSize {
    fn encode_into(&self, out: &mut Vec<u8>) {
        self.node.encode_into(out);
        out.extend(self.memory_usage.to_le_bytes());
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() < 8 {
            return Err(Error::new(ErrorKind::Other, "Wrong type"));
        }
        let node = RawTrieNode::decode(&bytes[0..bytes.len() - 8])?;
        let mut arr: [u8; 8] = Default::default();
        arr.copy_from_slice(&bytes[bytes.len() - 8..]);
        let memory_usage = u64::from_le_bytes(arr);
        Ok(RawTrieNodeWithSize { node, memory_usage })
    }
}

pub fn verify_state_proof(
    key: &[u8],
    nodes: &Vec<RawTrieNodeWithSize>,
    value: &[u8],
    state_root: &CryptoHash,
) -> Result<(), StateProofVerificationError> {
    let mut v = Vec::new();
    let mut hash_node = |node: &RawTrieNodeWithSize| {
        v.clear();
        node.encode_into(&mut v);
        CryptoHash(sha256(&v))
    };
    let mut key = NibbleSlice::new(key);
    let mut expected_hash = state_root.clone();
    let mut node_index: u16 = 0;

    for node in nodes.iter() {
        match node {
            RawTrieNodeWithSize {
                node: RawTrieNode::Leaf(node_key, _, value_hash),
                ..
            } => {
                if hash_node(&node) != expected_hash {
                    return Err(StateProofVerificationError::InvalidLeafNodeHash {
                        proof_index: node_index,
                    });
                }

                let nib = &NibbleSlice::from_encoded(&node_key).0;
                if &key != nib {
                    return Err(StateProofVerificationError::InvalidLeafNodeKey {
                        proof_index: node_index,
                    });
                }

                match CryptoHash(sha256(value)) == *value_hash {
                    true => return Ok(()),
                    false => {
                        return Err(StateProofVerificationError::InvalidLeafNodeValueHash {
                            proof_index: node_index,
                        })
                    }
                }
            }
            RawTrieNodeWithSize {
                node: RawTrieNode::Extension(node_key, child_hash),
                ..
            } => {
                if hash_node(&node) != expected_hash {
                    return Err(StateProofVerificationError::InvalidExtensionNodeHash {
                        proof_index: node_index,
                    });
                }
                expected_hash = *child_hash;

                let nib = NibbleSlice::from_encoded(&node_key).0;
                if !key.starts_with(&nib) {
                    return Err(StateProofVerificationError::InvalidExtensionNodeKey {
                        proof_index: node_index,
                    });
                }
                key = key.mid(nib.len());
            }
            RawTrieNodeWithSize {
                node: RawTrieNode::Branch(children, node_value),
                ..
            } => {
                if hash_node(&node) != expected_hash {
                    return Err(StateProofVerificationError::InvalidBranchNodeHash {
                        proof_index: node_index,
                    });
                }

                if key.is_empty() {
                    let maybe_value_hash = node_value.map(|x| x.1);
                    let expected_value_hash = CryptoHash(sha256(value));
                    return match maybe_value_hash {
                        Some(value_hash) => match expected_value_hash == value_hash {
                            true => Ok(()),
                            false => Err(StateProofVerificationError::InvalidBranchNodeValueHash {
                                proof_index: node_index,
                            }),
                        },
                        None => Err(StateProofVerificationError::MissingBranchNodeValue {
                            proof_index: node_index,
                        }),
                    };
                }
                let index = key.at(0);
                match &children[index as usize] {
                    Some(child_hash) => {
                        key = key.mid(1);
                        expected_hash = *child_hash;
                    }
                    None => {
                        return Err(StateProofVerificationError::MissingBranchNodeChildHash {
                            proof_index: node_index,
                        })
                    }
                }
            }
        }
        node_index += 1;
    }
    Err(StateProofVerificationError::InvalidProofDataLength)
}
