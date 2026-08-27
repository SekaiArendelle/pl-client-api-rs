use serde::Serialize;
use serde_json::Value;

/// A community content category understood by the Physics-Lab API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Category {
    Experiment,
    Discussion,
}

/// A community tag understood by the Physics-Lab API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Tag {
    #[serde(rename = "知识库")]
    KnowledgeBase,
    #[serde(rename = "精选")]
    Featured,
    #[serde(rename = "小学")]
    ElementarySchool,
    #[serde(rename = "高中")]
    HighSchool,
    #[serde(rename = "初中")]
    MiddleSchool,
    #[serde(rename = "大学")]
    College,
    #[serde(rename = "专科")]
    Professional,
    #[serde(rename = "娱乐实验")]
    FunExperiment,
    #[serde(rename = "小作品")]
    SmallProject,
    #[serde(rename = "教学实验")]
    Curricular,
    #[serde(rename = "禁止改编")]
    NoRemixes,
    #[serde(rename = "精选申请")]
    ApplyForFeature,
    #[serde(rename = "BUG")]
    Bug,
    #[serde(rename = "交流")]
    Discussion,
    #[serde(rename = "小说专区")]
    Stories,
    #[serde(rename = "聊天")]
    Chatroom,
    #[serde(rename = "问与答")]
    QuestionAndAnswer,
    #[serde(rename = "逻辑电路")]
    LogicCircuit,
    #[serde(rename = "直流电路")]
    DcCircuit,
    #[serde(rename = "交流电路")]
    AcCircuit,
    #[serde(rename = "电子电路")]
    Electronic,
    #[serde(rename = "兴趣")]
    Interest,
}

/// The user fields included in the authentication response.
#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub id: String,
    pub nickname: Option<String>,
    pub signature: Option<String>,
    pub is_bound: bool,
    pub gold: i64,
    pub level: i64,
    pub avatar: i64,
    pub avatar_region: i64,
    pub decoration: i64,
    pub verification: Value,
}
