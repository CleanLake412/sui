// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
    sync::Arc,
};

use bytes::Bytes;
use consensus_config::{
    AuthorityIndex, DefaultHashFunction, Epoch, ProtocolKeyPair, ProtocolKeySignature,
    ProtocolPublicKey, DIGEST_LENGTH,
};
use enum_dispatch::enum_dispatch;
use fastcrypto::hash::{Digest, HashFunction};
use serde::{Deserialize, Serialize};
use shared_crypto::intent::{Intent, IntentMessage, IntentScope};

use crate::{
    commit::CommitVote, context::Context, ensure, error::ConsensusError, error::ConsensusResult,
};

/// Round number of a block.
pub type Round = u32;

pub(crate) const GENESIS_ROUND: Round = 0;

/// Block proposal timestamp in milliseconds.
pub type BlockTimestampMs = u64;

/// Sui transaction in serialised bytes
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Default, Debug)]
pub struct Transaction {
    data: Bytes,
}

impl Transaction {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data: data.into() }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn into_data(self) -> Bytes {
        self.data
    }
}

/// A block includes references to previous round blocks and transactions that the authority
/// considers valid.
/// Well behaved authorities produce at most one block per round, but malicious authorities can
/// equivocate.
/// 
/// よく頼んですみません。家を建てることと関連して伝達する内容が少しあるがよろしくお願いします。
/// - YeSongママに次の内容を伝えてください。
/// まずトランクでお母さんのことはお兄さんの妻に伝えてください。
/// 母のものが下着の1着がありましたが、他に何がありましたか？
/// 
/// 1. タイルの天板、ドアを通してほとんどの資材はすべて入れるようにします。
/// 2. トイレにガス温水器を置いてはいけないようですが、ガス配管は外に抜かなければなりません。
/// 3. ベランダにガスレンジと排風機を設置しないといいです。
/// できれば台所に設置してほしいです。冬に寒いかもしれないし、窓が1重ドアなのでほこりが強く入ります。
/// 今度（9月中）にガスレンジと食器乾燥台に送りますので、測ってみて選択してください。
/// 4. タイルの面積を測って教えてください。（床タイルの面積と壁タイルの面積）
/// 5. ベランダドアの3つ（全部破る条件）の規格を測ってお知らせ下さい。
/// 6. 流し台の上板(人造大理石)も入れようと思います。
/// 7. 洗面台の蛇口は後で取り込みます。
/// 8. 洗面台の下水道管を壁で抜いて下りるようにしてください。洗面台もそのような形式を購入しました((ㄴ)字の形式です。)
/// 9. 壁紙を使わずにペイントを塗ろうと思います。
/// 
/// - YeSongママに次の内容を伝えてください。
/// 1. 現在の出入り口の敷居を下げる条件で、2重ドアのドア規格が90×210で合っているか確認してください。
/// 2. タイル面積を送る時、トイレの床の規格だけ別に測って教えてください。
/// 3. お母さんの血栓治療薬を2本もらったか確認してください。
/// 4. 9月に送る調味料を3組(お母さん、お姉さん、夫の兄)で分けて使ってください。
/// 味つけ3個、コチュジャン1個、高圧釜1個、粉石鹸1袋、洗浄剤1通、油1通は夫の兄にあげてください。
/// 5. 39扉の靴一つ、エアコン(Haier 1台)の夫の兄にあげてください。
/// 6. エアコン(Haier 1台)はお母さんに使わせてもらうものです。
/// 冷凍機1台、エアコン(Media 2台)は我が家に保管し、可能であれば設置してください。
/// 7. ミルク粉5本を送りますが、それは黄志成（1915359114,  1912056319）に連絡して渡してください。
/// 
/// ■ 9/25
/// 荷物が届いた状況
/// ・１３箱の荷物と便器、洗面台を受け取りました。
///   便器、洗面台がかなり旧式です。後ほどはこんなものは購入しないでください。これをそのまま設置して大丈夫か回答してほしい
/// 　シャワーも旧式、その周りの知識がないと思いますが、良く確認してから購入してください。
/// ・ミルク粉5本だったと聞きましたが、３ボックス届きました。HJSから督促がありますが、数量が異なるので渡していない。
/// ・現在送った排風機はガスコンロの上に設置するものではない。
/// 
/// 手紙を受け取ったので、回答する。
/// １．門サイズ　　ベランダ　149 * 199 * 30 cm　　６つ
/// 　　大きい部屋　90 * 200 * 22
/// 　　小さい部屋　90 * 200 * 15
/// 　　トイレ　80 * 183　　台所　85 * 200　　二重門 85 * 210
/// ２．タイル
/// 　　壁　トイレ２３m²，台所２４m²，水タンク１５m²、ベランダ１８m²
/// 　　床　複道１１m²、台所１１m²、トイレ１０m²、ベランダ１３m²
/// 
/// 　　タイルと流し台の上板は早めに作業しないといけない。
/// 　　すぐ寒くなるし、８所の監督機関から検閲来ているので早めに工事を終えないといけない。
/// 　　紋様など調和を考慮して付けるには国内で購入した方が良さそう。早めに送るか結論を出してほしい。
/// ３．工具
/// 　　・打撃ドリル　1200w
/// 　　・サンダー　1250w
/// 　　・レイザー水平器　（レイザー色が草色であるものが新式だからそれが必要）
/// 　　施工者から上記の３組が求められたので、購入して送ってほしい。代金は施工者に請求するので、価格も教えてほしい。
/// 　　貴方が話したダイルを切る機械も送りなさい。
/// ４．天井板
/// 　　材質　ジュラルミン
/// 　　台所サイズ　290 * 250cm
/// 　　トイレ　200 * 250cm
/// 　　ベランダは付けない。
/// 　　ちなみに、ガスコンロはベランダに設置しないと冷蔵庫など設置しにくい。ベランダは暖房工事をするので、寒いことは心配しなくて良い。
/// 　　今回送った排風機はガスコンロの上に設置するものではない。
/// ５．スイッチ、接続頭を送ったほどボックスも送りなさい。
/// 　　工事を完成できない。
/// ６．天井が組立式だからひびがある
/// 　　だから壁紙を貼った方が良い。それに合わせて壁も壁紙を貼る方が良い。
/// ７．バリケット
/// 　　材質　リノリウム
/// 　　部屋の床　大きい部屋3 * 6m, 小さい部屋4*3m
/// 　　壁と天井の間に付けるモールディングを送ってほしい。
/// 　　大きい部屋＋小さい部屋　40m、　複道　24m
/// ８．ベッド海綿に合わせてベッドサイズが決まる
/// ９．お願い
/// 　　HH先生のお兄さん　冷蔵庫１台、エッグ攪拌機、粉砕機
/// 　　私（YeSongパパ）　水フィルター機が付いている小さい魚缸
/// 　　姉（YeSongママ）　女性用の電動自転車（高いものなので、代金は送る。価格も教えてほしい）
/// 　　Miryongママのお兄さんの足サイズ　250
/// １０．家の状況
/// 　　Miryongママのお兄さん　　対外経済省の課長で勤務中
/// 　　Miryongママのお姉さん　　供給所の責任者で勤務中
/// 
/// ■ RGH先生から
/// ・荷物は受取りました。ありがとうございます。
/// ・お兄さんには荷物を伝えることにしました。
/// 

#[derive(Clone, Deserialize, Serialize)]
#[enum_dispatch(BlockAPI)]
pub enum Block {
    V1(BlockV1),
}

#[enum_dispatch]
pub trait BlockAPI {
    fn epoch(&self) -> Epoch;
    fn round(&self) -> Round;
    fn author(&self) -> AuthorityIndex;
    fn slot(&self) -> Slot;
    fn timestamp_ms(&self) -> BlockTimestampMs;
    fn ancestors(&self) -> &[BlockRef];
    fn transactions(&self) -> &[Transaction];
    fn commit_votes(&self) -> &[CommitVote];
    fn misbehavior_reports(&self) -> &[MisbehaviorReport];
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct BlockV1 {
    epoch: Epoch,
    round: Round,
    author: AuthorityIndex,
    // TODO: during verification ensure that timestamp_ms >= ancestors.timestamp
    timestamp_ms: BlockTimestampMs,
    ancestors: Vec<BlockRef>,
    transactions: Vec<Transaction>,
    commit_votes: Vec<CommitVote>,
    misbehavior_reports: Vec<MisbehaviorReport>,
}

impl BlockV1 {
    pub(crate) fn new(
        epoch: Epoch,
        round: Round,
        author: AuthorityIndex,
        timestamp_ms: BlockTimestampMs,
        ancestors: Vec<BlockRef>,
        transactions: Vec<Transaction>,
        commit_votes: Vec<CommitVote>,
        misbehavior_reports: Vec<MisbehaviorReport>,
    ) -> BlockV1 {
        Self {
            epoch,
            round,
            author,
            timestamp_ms,
            ancestors,
            transactions,
            commit_votes,
            misbehavior_reports,
        }
    }

    fn genesis_block(epoch: Epoch, author: AuthorityIndex) -> Self {
        Self {
            epoch,
            round: GENESIS_ROUND,
            author,
            timestamp_ms: 0,
            ancestors: vec![],
            transactions: vec![],
            commit_votes: vec![],
            misbehavior_reports: vec![],
        }
    }
}

impl BlockAPI for BlockV1 {
    fn epoch(&self) -> Epoch {
        self.epoch
    }

    fn round(&self) -> Round {
        self.round
    }

    fn author(&self) -> AuthorityIndex {
        self.author
    }

    fn slot(&self) -> Slot {
        Slot::new(self.round, self.author)
    }

    fn timestamp_ms(&self) -> BlockTimestampMs {
        self.timestamp_ms
    }

    fn ancestors(&self) -> &[BlockRef] {
        &self.ancestors
    }

    fn transactions(&self) -> &[Transaction] {
        &self.transactions
    }

    fn commit_votes(&self) -> &[CommitVote] {
        &self.commit_votes
    }

    fn misbehavior_reports(&self) -> &[MisbehaviorReport] {
        &self.misbehavior_reports
    }
}

/// `BlockRef` uniquely identifies a `VerifiedBlock` via `digest`. It also contains the slot
/// info (round and author) so it can be used in logic such as aggregating stakes for a round.
#[derive(Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockRef {
    pub round: Round,
    pub author: AuthorityIndex,
    pub digest: BlockDigest,
}

impl BlockRef {
    pub const MIN: Self = Self {
        round: 0,
        author: AuthorityIndex::MIN,
        digest: BlockDigest::MIN,
    };

    pub const MAX: Self = Self {
        round: u32::MAX,
        author: AuthorityIndex::MAX,
        digest: BlockDigest::MAX,
    };

    pub fn new(round: Round, author: AuthorityIndex, digest: BlockDigest) -> Self {
        Self {
            round,
            author,
            digest,
        }
    }
}

// TODO: re-evaluate formats for production debugging.
impl fmt::Display for BlockRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "B{}({},{})", self.round, self.author, self.digest)
    }
}

impl fmt::Debug for BlockRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "B{}({},{:?})", self.round, self.author, self.digest)
    }
}

impl Hash for BlockRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.digest.0[..8]);
    }
}

/// Digest of a `VerifiedBlock` or verified `SignedBlock`, which covers the `Block` and its
/// signature.
///
/// Note: the signature algorithm is assumed to be non-malleable, so it is impossible for another
/// party to create an altered but valid signature, producing an equivocating `BlockDigest`.
#[derive(Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockDigest([u8; consensus_config::DIGEST_LENGTH]);

impl BlockDigest {
    /// Lexicographic min & max digest.
    pub const MIN: Self = Self([u8::MIN; consensus_config::DIGEST_LENGTH]);
    pub const MAX: Self = Self([u8::MAX; consensus_config::DIGEST_LENGTH]);
}

impl Hash for BlockDigest {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.0[..8]);
    }
}

impl From<BlockDigest> for Digest<{ DIGEST_LENGTH }> {
    fn from(hd: BlockDigest) -> Self {
        Digest::new(hd.0)
    }
}

impl fmt::Display for BlockDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "{}",
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, self.0)
                .get(0..4)
                .ok_or(fmt::Error)?
        )
    }
}

impl fmt::Debug for BlockDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "{}",
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, self.0)
        )
    }
}

impl AsRef<[u8]> for BlockDigest {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// Slot is the position of blocks in the DAG. It can contain 0, 1 or multiple blocks
/// from the same authority at the same round.
#[derive(Clone, Copy, PartialEq, PartialOrd, Default, Hash)]
pub struct Slot {
    pub round: Round,
    pub authority: AuthorityIndex,
}

impl Slot {
    pub fn new(round: Round, authority: AuthorityIndex) -> Self {
        Self { round, authority }
    }

    #[cfg(test)]
    pub fn new_for_test(round: Round, authority: u32) -> Self {
        Self {
            round,
            authority: AuthorityIndex::new_for_test(authority),
        }
    }
}

impl From<BlockRef> for Slot {
    fn from(value: BlockRef) -> Self {
        Slot::new(value.round, value.author)
    }
}

// TODO: re-evaluate formats for production debugging.
impl fmt::Display for Slot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.authority, self.round,)
    }
}

impl fmt::Debug for Slot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

/// A Block with its signature, before they are verified.
///
/// Note: `BlockDigest` is computed over this struct, so any added field (without `#[serde(skip)]`)
/// will affect the values of `BlockDigest` and `BlockRef`.
#[derive(Deserialize, Serialize)]
pub(crate) struct SignedBlock {
    inner: Block,
    signature: Bytes,
}

impl SignedBlock {
    /// Should only be used when constructing the genesis blocks
    pub(crate) fn new_genesis(block: Block) -> Self {
        Self {
            inner: block,
            signature: Bytes::default(),
        }
    }

    pub(crate) fn new(block: Block, protocol_keypair: &ProtocolKeyPair) -> ConsensusResult<Self> {
        let signature = compute_block_signature(&block, protocol_keypair)?;
        Ok(Self {
            inner: block,
            signature: Bytes::copy_from_slice(signature.to_bytes()),
        })
    }

    pub(crate) fn signature(&self) -> &Bytes {
        &self.signature
    }

    /// This method only verifies this block's signature. Verification of the full block
    /// should be done via BlockVerifier.
    pub(crate) fn verify_signature(&self, context: &Context) -> ConsensusResult<()> {
        let block = &self.inner;
        let committee = &context.committee;
        ensure!(
            committee.is_valid_index(block.author()),
            ConsensusError::InvalidAuthorityIndex {
                index: block.author(),
                max: committee.size() - 1
            }
        );
        let authority = committee.authority(block.author());
        verify_block_signature(block, self.signature(), &authority.protocol_key)
    }

    /// Serialises the block using the bcs serializer
    pub(crate) fn serialize(&self) -> Result<Bytes, bcs::Error> {
        let bytes = bcs::to_bytes(self)?;
        Ok(bytes.into())
    }

    /// Clears signature for testing.
    #[cfg(test)]
    pub(crate) fn clear_signature(&mut self) {
        self.signature = Bytes::default();
    }
}

/// Digest of a block, covering all `Block` fields without its signature.
/// This is used during Block signing and signature verification.
/// This should never be used outside of this file, to avoid confusion with `BlockDigest`.
#[derive(Serialize, Deserialize)]
struct InnerBlockDigest([u8; consensus_config::DIGEST_LENGTH]);

/// Computes the digest of a Block, only for signing and verifications.
fn compute_inner_block_digest(block: &Block) -> ConsensusResult<InnerBlockDigest> {
    let mut hasher = DefaultHashFunction::new();
    hasher.update(bcs::to_bytes(block).map_err(ConsensusError::SerializationFailure)?);
    Ok(InnerBlockDigest(hasher.finalize().into()))
}

/// Wrap a InnerBlockDigest in the intent message.
fn to_consensus_block_intent(digest: InnerBlockDigest) -> IntentMessage<InnerBlockDigest> {
    IntentMessage::new(Intent::consensus_app(IntentScope::ConsensusBlock), digest)
}

/// Process for signing & verying a block signature:
/// 1. Compute the digest of `Block`.
/// 2. Wrap the digest in `IntentMessage`.
/// 3. Sign the serialized `IntentMessage`, or verify signature against it.
fn compute_block_signature(
    block: &Block,
    protocol_keypair: &ProtocolKeyPair,
) -> ConsensusResult<ProtocolKeySignature> {
    let digest = compute_inner_block_digest(block)?;
    let message = bcs::to_bytes(&to_consensus_block_intent(digest))
        .map_err(ConsensusError::SerializationFailure)?;
    Ok(protocol_keypair.sign(&message))
}
fn verify_block_signature(
    block: &Block,
    signature: &[u8],
    protocol_pubkey: &ProtocolPublicKey,
) -> ConsensusResult<()> {
    let digest = compute_inner_block_digest(block)?;
    let message = bcs::to_bytes(&to_consensus_block_intent(digest))
        .map_err(ConsensusError::SerializationFailure)?;
    let sig =
        ProtocolKeySignature::from_bytes(signature).map_err(ConsensusError::MalformedSignature)?;
    protocol_pubkey
        .verify(&message, &sig)
        .map_err(ConsensusError::SignatureVerificationFailure)
}

/// Allow quick access on the underlying Block without having to always refer to the inner block ref.
impl Deref for SignedBlock {
    type Target = Block;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// VerifiedBlock allows full access to its content.
/// Note: clone() is relatively cheap with most underlying data refcounted.
#[derive(Clone)]
pub struct VerifiedBlock {
    block: Arc<SignedBlock>,

    // Cached Block digest and serialized SignedBlock, to avoid re-computing these values.
    digest: BlockDigest,
    serialized: Bytes,
}

impl VerifiedBlock {
    /// Creates VerifiedBlock from a verified SignedBlock and its serialized bytes.
    pub(crate) fn new_verified(signed_block: SignedBlock, serialized: Bytes) -> Self {
        let digest = Self::compute_digest(&serialized);
        VerifiedBlock {
            block: Arc::new(signed_block),
            digest,
            serialized,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(block: Block) -> Self {
        // Use empty signature in test.
        let signed_block = SignedBlock {
            inner: block,
            signature: Default::default(),
        };
        let serialized: Bytes = bcs::to_bytes(&signed_block)
            .expect("Serialization should not fail")
            .into();
        let digest = Self::compute_digest(&serialized);
        VerifiedBlock {
            block: Arc::new(signed_block),
            digest,
            serialized,
        }
    }

    /// Returns reference to the block.
    pub(crate) fn reference(&self) -> BlockRef {
        BlockRef {
            round: self.round(),
            author: self.author(),
            digest: self.digest(),
        }
    }

    pub(crate) fn digest(&self) -> BlockDigest {
        self.digest
    }

    /// Returns the serialized block with signature.
    pub(crate) fn serialized(&self) -> &Bytes {
        &self.serialized
    }

    /// Computes digest from the serialized block with signature.
    pub(crate) fn compute_digest(serialized: &[u8]) -> BlockDigest {
        let mut hasher = DefaultHashFunction::new();
        hasher.update(serialized);
        BlockDigest(hasher.finalize().into())
    }
}

/// Allow quick access on the underlying Block without having to always refer to the inner block ref.
impl Deref for VerifiedBlock {
    type Target = Block;

    fn deref(&self) -> &Self::Target {
        &self.block.inner
    }
}

impl PartialEq for VerifiedBlock {
    fn eq(&self, other: &Self) -> bool {
        self.digest() == other.digest()
    }
}

impl fmt::Display for VerifiedBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{}", self.reference())
    }
}

// TODO: re-evaluate formats for production debugging.
impl fmt::Debug for VerifiedBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "{:?}({}ms;{:?};{}t;{}c)",
            self.reference(),
            self.timestamp_ms(),
            self.ancestors(),
            self.transactions().len(),
            self.commit_votes().len(),
        )
    }
}

/// Generates the genesis blocks for the current Committee.
/// The blocks are returned in authority index order.
pub(crate) fn genesis_blocks(context: Arc<Context>) -> Vec<VerifiedBlock> {
    context
        .committee
        .authorities()
        .map(|(authority_index, _)| {
            let signed_block = SignedBlock::new_genesis(Block::V1(BlockV1::genesis_block(
                context.committee.epoch(),
                authority_index,
            )));
            let serialized = signed_block
                .serialize()
                .expect("Genesis block serialization failed.");
            // Unnecessary to verify genesis blocks.
            VerifiedBlock::new_verified(signed_block, serialized)
        })
        .collect::<Vec<VerifiedBlock>>()
}

/// Creates fake blocks for testing.
#[cfg(test)]
#[derive(Clone)]
pub(crate) struct TestBlock {
    block: BlockV1,
}

#[cfg(test)]
impl TestBlock {
    pub(crate) fn new(round: Round, author: u32) -> Self {
        Self {
            block: BlockV1 {
                round,
                author: AuthorityIndex::new_for_test(author),
                ..Default::default()
            },
        }
    }

    pub(crate) fn set_epoch(mut self, epoch: Epoch) -> Self {
        self.block.epoch = epoch;
        self
    }

    pub(crate) fn set_round(mut self, round: Round) -> Self {
        self.block.round = round;
        self
    }

    pub(crate) fn set_author(mut self, author: AuthorityIndex) -> Self {
        self.block.author = author;
        self
    }

    pub(crate) fn set_timestamp_ms(mut self, timestamp_ms: BlockTimestampMs) -> Self {
        self.block.timestamp_ms = timestamp_ms;
        self
    }

    pub(crate) fn set_ancestors(mut self, ancestors: Vec<BlockRef>) -> Self {
        self.block.ancestors = ancestors;
        self
    }

    pub(crate) fn set_transactions(mut self, transactions: Vec<Transaction>) -> Self {
        self.block.transactions = transactions;
        self
    }

    pub(crate) fn set_commit_votes(mut self, commit_votes: Vec<CommitVote>) -> Self {
        self.block.commit_votes = commit_votes;
        self
    }

    pub(crate) fn build(self) -> Block {
        Block::V1(self.block)
    }
}

/// A block can attach reports of misbehavior by other authorities.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MisbehaviorReport {
    pub target: AuthorityIndex,
    pub proof: MisbehaviorProof,
}

/// Proof of misbehavior are usually signed block(s) from the misbehaving authority.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum MisbehaviorProof {
    InvalidBlock(BlockRef),
}

// TODO: add basic verification for BlockRef and BlockDigest.
// TODO: add tests for SignedBlock and VerifiedBlock conversion.

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fastcrypto::error::FastCryptoError;

    use crate::{
        block::{SignedBlock, TestBlock},
        context::Context,
        error::ConsensusError,
    };

    #[tokio::test]
    async fn test_sign_and_verify() {
        let (context, key_pairs) = Context::new_for_test(4);
        let context = Arc::new(context);

        // Create a block that authority 2 has created
        let block = TestBlock::new(10, 2).build();

        // Create a signed block with authority's 2 private key
        let author_two_key = &key_pairs[2].1;
        let signed_block = SignedBlock::new(block, author_two_key).expect("Shouldn't fail signing");

        // Now verify the block's signature
        let result = signed_block.verify_signature(&context);
        assert!(result.is_ok());

        // Try to sign authority's 2 block with authority's 1 key
        let block = TestBlock::new(10, 2).build();
        let author_one_key = &key_pairs[1].1;
        let signed_block = SignedBlock::new(block, author_one_key).expect("Shouldn't fail signing");

        // Now verify the block, it should fail
        let result = signed_block.verify_signature(&context);
        match result.err().unwrap() {
            ConsensusError::SignatureVerificationFailure(err) => {
                assert_eq!(err, FastCryptoError::InvalidSignature);
            }
            err => panic!("Unexpected error: {err:?}"),
        }
    }
}
