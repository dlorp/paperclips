use crate::cli::SchemaTarget;
use crate::error;
use serde_json::{Value, json};

pub fn contract(target: SchemaTarget) -> Value {
    let records = json!({
        "cut": {"kind":"cut","id":"pc_<12 lowercase hex>","ts":"RFC3339 UTC milliseconds","agent":"string","text":"string <= 10000 bytes","tags":["string"],"severity":"minor|major|blocker","cwd":"repo-relative path or absolute if no repo; optional when folding records from other writers","repo":"\".\" when inside a repo, null otherwise","where":"string; omitted when unset","recurrence_of":"first matching cut id; omitted when this is not a recurrence"},
        "resolve": {"kind":"resolve","id":"pc_<12 lowercase hex>","ts":"RFC3339 UTC milliseconds","agent":"string","note":"string|null"},
        "feedback": {"feedback_id":"pc_... or cl_...","kind":"cut|clip","disposition":"resolved|wontfix|duplicate|already_fixed|deferred","resolution":"string","linked_task":"string|null","resolved_at":"RFC3339 UTC milliseconds","resolved_by":"string","verification_method":"string","verification_result":"string","verified_at":"RFC3339 UTC milliseconds","change_class":"code|standard|cron|skill|doc|config|null","changed_artifact":"string|null","channel":"kanban|review|discord|null","task_id":"string|null","owner":"string|null","reason":"string|null","recorded_at":"first disposition timestamp","updated_at":"latest disposition timestamp|null"},
        "list_item": {"cut":"all cut fields","status":"open|resolved|wontfix","resolution":"{ts,agent,note}|omitted"}
    });
    let errors = json!({
        "shape": {"ok":false,"error":{"code":"string","message":"string","details":{},"retryable":false,"suggested_fix":"string"},"meta":{"contract":1}},
        "codes": error::error_codes()
    });
    let exit_codes: Value = json!(error::exit_code_map());
    match target {
        SchemaTarget::Record => json!({"contract":1,"records":records}),
        SchemaTarget::Error => json!({"contract":1,"errors":errors}),
        SchemaTarget::ExitCodes => json!({"contract":1,"exit_codes":exit_codes}),
        SchemaTarget::All => json!({
            "contract": 1,
            "success_envelope": {"ok":true,"data":"command-specific object","meta":{"contract":1,"file":"absolute path where relevant","agent_source":"flag|env|detected|default where relevant","warnings":["string; omitted when empty"]}},
            "commands": {
                "add": {"alias":["log"],"positional":"TEXT or -; optional when stdin is piped","flags":{"--agent":"NAME","--tag":"TAG; repeatable","--severity":"minor|major|blocker; default minor","--dry-run":"boolean","--force":"bypass secret scan"},"output":"{changed,record}","read_only":false,"appends":true,"destructive":false},
                "list": {"flags":{"--status":"open|resolved|wontfix|all; default open","--agent":"NAME","--tag":"TAG","--severity":"minor|major|blocker","--since":"full RFC3339|Nd|Nh","--limit":"N; default 50","--format":"json|md; default json","--unresolved":"boolean; no terminal disposition in the feedback ledger","--digest":"boolean; open-cut severity counts"},"output":"{items,count,total,truncated,digest?}; md is raw markdown","read_only":true,"appends":false,"destructive":false},
                "resolve": {"positional":"ID; optional pc_ plus at least 4 hex digits","flags":{"--note":"TEXT","--agent":"NAME","--dry-run":"boolean","--resolution":"TEXT; CAPA required unless --change","--linked-task":"TASK|null","--resolved-by":"NAME; CAPA required unless --owner","--verification-method":"TEXT; CAPA required","--verification-result":"TEXT; CAPA required","--change":"TEXT; legacy, maps to resolution","--class":"code|standard|cron|skill|doc|config; legacy","--channel":"kanban|review|discord; legacy","--task":"TASK; legacy, maps to linked_task","--owner":"NAME; legacy, maps to resolved_by"},"output":"{changed,record-with-resolution,feedback}","read_only":false,"appends":true,"destructive":false},
                "wontfix": {"positional":"ID; optional pc_ plus at least 4 hex digits","flags":{"--reason":"TEXT; required","--owner":"NAME","--task":"TASK"},"output":"{changed,feedback}","read_only":false,"appends":true,"destructive":false},
                "schema": {"positional":"all|record|error|exit-codes; default all","read_only":true,"appends":false,"destructive":false},
                "doctor": {"flags":{},"output":"{healthy,findings,checked_lines}","exit_codes":{"0":"healthy","1":"findings"},"read_only":true,"appends":false,"destructive":false}
            },
            "global_flags": ["--file <PATH>","--pretty"],
            "env": {
                "PAPERCUTS_FILE":"log-file override",
                "PAPERCUTS_AGENT":"agent-name fallback",
                "PAPERCUTS_NOW":"full RFC3339 clock override",
                "FEEDBACK_RESOLUTIONS_FILE":"disposition-ledger override"
            },
            "records": records,
            "id": {"prefix":"pc_","hex_digits":12,"hash":"SHA-256 first 6 bytes","fields_in_order":["ts","agent","text","severity","sorted tags joined with comma"],"encoding":"u32 little-endian UTF-8 byte length before each field"},
            "discovery": ["--file","PAPERCUTS_FILE","nearest .git directory or file then <root>/.papercuts.jsonl","$HOME/.papercuts/log.jsonl"],
            "feedback_discovery": ["FEEDBACK_RESOLUTIONS_FILE","next to an explicit --file","<repo>/.feedback-resolutions.jsonl","$HOME/.papercuts/feedback-resolutions.jsonl"],
            "errors": errors,
            "exit_codes": exit_codes,
            "storage": {"format":"append-only JSONL","locking":"local filesystems only; 50 retries x 100ms","durability":"best effort; no fsync per append"}
        }),
    }
}
