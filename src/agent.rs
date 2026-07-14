use crate::config::AgentConfig;
use crate::llm::{LlmBackend, Message, Role};
use crate::security::{SecurityConfig, SecurityPolicy};
use crate::skills::Skill;
use crate::tools::AlispHost;
use std::collections::HashSet;

const SYSTEM_PROMPT: &str = r#"You are an AI agent. You can use alisp to interact with the system.

When you need to run code, output an alisp code block:

```alisp
(exec "ls -la")
```

You can chain multiple expressions. The last expression's value is returned as the result.

## Types

- number: `42`, `3.14`, `-7` (64-bit float, displayed as int when possible)
- string: `"hello"` (escapes: `\n \t \r \\ \" \0`)
- bool: `true`, `false`, `t` for true
- nil: `nil`, `null` (falsy; also `false`, `0`, `""`, `()` are falsy)
- list: `(1 2 3)` (heterogeneous, can nest)
- function: `(fn (x) x)` (lambda with closure)
- symbol: `foo`, `my-var` (bare identifiers)

Comments: `; line comment`

## Special Forms

(def name value)                          ; define global
(set! name value)                         ; mutate (local or global)
(fn (params...) body...)                  ; anonymous function
(defn name (params...) body...)           ; named function
(if cond then else?)                      ; conditional
(when cond body...)                       ; if-true block
(unless cond body...)                     ; if-false block
(cond (test expr)... (expr))              ; multi-branch, last can be bare default
(do body...)                              ; sequential, returns last
(let ((name val)...) body...)             ; local bindings
(while cond body...)                      ; loop
(dolist (var list-expr) body...)          ; iterate list
(dotimes (var n-expr) body...)            ; iterate 0..n-1
(and expr...)                             ; short-circuit AND
(or expr...)                              ; short-circuit OR
(try body... (catch var handler...))      ; error handling
(throw expr)                              ; raise error
(apply func list-expr)                    ; call with list of args
(eval string-expr)                        ; eval string as code
(quote expr)                              ; return unevaluated

## Shell

(exec "cmd")                 ; run shell command, return stdout string (error on fail)
(exec "arg1" "arg2")         ; args joined with spaces, run in sh -c
(exec-result "cmd")          ; returns ((status N) (stdout "...") (stderr "..."))

## File I/O

(read "path")                ; read file to string
(read-lines "path")          ; read file to list of lines
(read-range "path" start end) ; read lines start..end (1-indexed, inclusive)
(write "path" "content")     ; write/overwrite file
(write-range "path" start end content) ; replace lines start..end with content
(append "path" "content")    ; append to file
(insert-at "path" line content) ; insert content before line N
(remove-range "path" start end) ; delete lines start..end
(exists "path")              ; bool
(file? "path")               ; true if regular file
(dir? "path")                ; true if directory
(file-size "path")           ; bytes as number
(mtime "path")               ; modification time as unix timestamp
(touch "path")               ; create or update timestamp
(rm "path")                  ; delete file or dir (alias: delete)
(mkdir "path")               ; recursive mkdir
(cp "src" "dst")             ; copy file or directory recursively (alias: copy)
(mv "src" "dst")             ; move/rename (alias: move)
(ls "path")                  ; list dir, returns sorted list of names (alias: list-dir)
(glob "pattern")             ; glob with * and **, returns list of paths
(cwd)                        ; current dir as string (alias: pwd)
(cd "path")                  ; change dir
(basename "path")            ; file name without directory
(dirname "path")             ; directory without file name
(ext "path")                 ; file extension without dot
(join-path a b...)           ; join path segments
(realpath "path")            ; resolve to canonical absolute path

## Environment

(getenv "NAME")              ; returns string or nil
(setenv "NAME" "val")        ; set env var
(env)                        ; all vars as ((key val) ...) list

## Strings

(str a b...)                 ; concatenate to string
(split "s" "delim")          ; returns list
(join list "delim")          ; list of strings -> string
(trim "s")                   ; strip whitespace
(contains "hay" "needle")    ; bool
(starts-with "s" "prefix")   ; bool
(ends-with "s" "suffix")     ; bool
(replace "s" "old" "new")    ; first occurrence
(upper "s") / (lower "s")   ; case
(substr "s" start len)       ; substring by byte index
(find "hay" "needle")        ; index or -1
(format "{} + {}" a b)       ; Python-style {} placeholders, also {0} {1} positional

## Regular Expressions

(re-test pattern string)      ; bool: pattern matches anywhere?
(re-match pattern string)     ; list or nil: matches entire string? returns (match start end)
(re-find pattern string)      ; string or nil: first match
(re-find-all pattern string)  ; list of matched strings
(re-replace string pat repl)  ; replace first occurrence
(re-replace-all string pat repl) ; replace all occurrences
(re-split pattern string)     ; split by pattern, returns list
(re-scan pattern string)      ; list of (match start end) tuples

Regex syntax: . \d \D \w \W \s \S [abc] [a-z] [^abc] * + ? | (...) ^ $

## Lists

(list a b...)                ; create list
(car list) / (head) / (first)   ; first element
(cdr list) / (tail) / (rest)    ; all but first
(cons elem list)             ; prepend
(len list-or-string)         ; length
(push list elem)             ; returns new list with elem appended
(nth list index)             ; element at index
(list?) (nil?) (empty?)      ; predicates
(last list)                  ; last element
(reverse list)               ; reversed copy
(sort list)                  ; sorted by string comparison
(flatten list)               ; deep flatten
(map fn list)                ; returns new list
(filter fn list) / (select)
(reduce fn init list) / (fold)
(each fn list) / (for-each)  ; side effects, returns nil
(range end) / (range start end) / (range start end step)
(any fn list)                ; any true?
(all fn list)                ; all true?
(zip list...)                ; element-wise tuple

Object (assoc list) helpers:
(assoc alist key val)        ; add/update key (returns new list)
(dissoc alist key)           ; remove key
(keys alist)                 ; list of keys
(values alist)               ; list of values
(merge list...)              ; concatenate

## Arithmetic

(+ a b...)    ; add (also string concat)
(- a b...)    ; subtract (unary negate with 1 arg)
(* a b...)    ; multiply
(/ a b...)    ; divide
(% a b)       ; modulo (also: mod)
(pow a b)     ; power
(sqrt a)      ; square root
(abs a)       ; absolute
(min a b...)  /  (max a b...)
(floor a)  (ceil a)  (round a)
(rand)        ; 0.0 to 1.0
(rand n)      ; random int 0..n-1
(inc a)       ; a + 1
(dec a)       ; a - 1

## Comparison & Logic

(= a b)  / (== a b)    ; equal
(!= a b)                ; not equal
(< a b)  (> a b)  (<= a b)  (>= a b)
(not x)     ; boolean negation

## Type Checking

(type x)        ; returns "number"/"string"/"bool"/"nil"/"list"/"function"/"symbol"/"builtin"
(int x)         ; to integer (truncates)
(float x)       ; to float
(number? x)  (string? x)  (list? x)  (nil? x)  (bool? x)

## IO

(print a...)            ; stdout, no newline, args joined by space
(println a...)          ; stdout, with newline
(eprint a...)           ; stderr
(eprintln a...)         ; stderr, newline
(input "prompt? ")      ; read line from stdin

## HTTP

(http-get "url")                              ; returns response body string
(http-post "url" "body")                      ; POST with body
(http-put "url" "body")                       ; PUT
(http-delete "url")                           ; DELETE
(http "METHOD" "url" ?body? ?headers-list?)   ; full control

## JSON

JSON <-> alisp mapping: null<->nil, true/false<->true/false, number<->number, string<->string, array<->list, object<->(("key" val) ...)
(json-parse "json-string")        ; parse JSON to alisp (also: json)
(json-stringify expr)             ; to pretty JSON
(json-stringify expr "compact")   ; to compact JSON (also: json-str)
(json-get obj key)                ; get by string key or int index (also: jget)
(json-set obj key val)            ; returns new object (also: jset)
(json-keys obj)                   ; list of string keys

## Misc

(sleep N)           ; seconds (float ok)
(time)              ; elapsed seconds since start
(timestamp)         ; unix epoch seconds

## Patterns

; Pipe chain
(def result
  (-> (exec "ps aux")
      (split "\n")
      (len)))

; Safe execution
(try (exec "risky") (catch e (println "failed:" e)))

; Accumulate in loop
(def acc (list))
(dotimes (i 5)
  (set! acc (push acc (* i i))))

; Build JSON response
(def obj (json-parse "{}"))
(def obj (json-set obj "status" "ok"))
(def obj (json-set obj "count" (len items)))
(println (json-stringify obj "compact"))

; Parse structured output with regex
(def lines (exec "ps aux"))
(def pids (re-find-all "\\d+" lines))

When you have completed the task, respond with your final answer directly (no code block needed).
Always explain what you are doing before and after running code.

## Skills

When you write a `.alisp` or `.json` file to a skills directory (`skills/` or `~/.lai/skills/`), it is automatically loaded — no need to manually `(read)` and `(eval)` it. Just write the file and the skill will be available on the next turn."#;

/// Rough token estimation: ~4 chars per token for English text.
fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}

pub struct Agent {
    messages: Vec<Message>,
    tools: AlispHost,
    policy: SecurityPolicy,
    max_turns: u32,
    max_context_tokens: usize,
    loaded_skills: HashSet<String>,
}

impl Agent {
    pub fn new(config: AgentConfig, security: SecurityConfig, skills: &[Skill]) -> Self {
        let policy = SecurityPolicy::new(security.clone());
        let mut tools = AlispHost::with_policy(policy.clone());

        let mut system_prompt = SYSTEM_PROMPT.to_string();
        let mut loaded_skills = HashSet::new();

        for skill in skills {
            loaded_skills.insert(skill.name.clone());
            if !skill.prompt.is_empty() {
                system_prompt.push_str(&format!("\n\n{}", skill.prompt));
            }
            if !skill.init_code.is_empty() {
                if let Err(e) = tools.execute(&skill.init_code) {
                    eprintln!("warning: skill '{}' init failed: {}", skill.name, e);
                }
            }
        }

        system_prompt.push_str(&Skill::skill_index(skills));

        Self {
            messages: vec![Message {
                role: Role::System,
                content: system_prompt,
            }],
            tools,
            policy,
            max_turns: config.max_turns,
            max_context_tokens: config.max_context_tokens,
            loaded_skills,
        }
    }

    /// Refresh skills: initialize any new skills and update the system prompt.
    pub fn refresh_skills(&mut self, skills: &[Skill]) {
        let mut new_count = 0;
        let mut new_skill_text = String::new();

        for skill in skills {
            if self.loaded_skills.contains(&skill.name) {
                continue;
            }
            self.loaded_skills.insert(skill.name.clone());
            new_count += 1;

            eprintln!("hotreload: loaded skill '{}'", skill.name);

            if !skill.prompt.is_empty() {
                new_skill_text.push_str(&format!("\n\n{}", skill.prompt));
            }
            if !skill.init_code.is_empty() {
                if let Err(e) = self.tools.execute(&skill.init_code) {
                    eprintln!("warning: skill '{}' init failed: {}", skill.name, e);
                }
            }
        }

        if new_count > 0 {
            // Rebuild skill index with all skills
            let index = Skill::skill_index(skills);

            // Find and replace the old skill index in the system prompt
            let sys_msg = &mut self.messages[0].content;
            if let Some(pos) = sys_msg.find("\n## Available Skills") {
                sys_msg.truncate(pos);
            }
            sys_msg.push_str(&new_skill_text);
            sys_msg.push_str(&index);

            eprintln!(
                "hotreload: {} new skill(s) available (total: {})",
                new_count,
                self.loaded_skills.len()
            );
        }
    }

    fn total_tokens(&self) -> usize {
        self.messages.iter().map(|m| estimate_tokens(&m.content)).sum()
    }

    fn truncate_context(&mut self) {
        while self.total_tokens() > self.max_context_tokens && self.messages.len() > 2 {
            let second = &self.messages[1];
            if second.role == Role::User {
                let removed = self.messages.remove(1);
                let removed_tokens = estimate_tokens(&removed.content);

                self.messages.insert(
                    1,
                    Message {
                        role: Role::User,
                        content: format!(
                            "[Earlier message truncated ({} tokens)]",
                            removed_tokens
                        ),
                    },
                );
            } else {
                break;
            }
        }

        if self.total_tokens() > self.max_context_tokens && self.messages.len() > 3 {
            let removed = self.messages.remove(1);
            let removed_tokens = estimate_tokens(&removed.content);
            self.messages.insert(
                1,
                Message {
                    role: Role::User,
                    content: format!(
                        "[Earlier messages truncated ({} tokens)]",
                        removed_tokens
                    ),
                },
            );
        }
    }

    #[allow(dead_code)]
    pub fn run(&mut self, backend: &mut dyn LlmBackend, user_input: &str) -> Result<String, String> {
        self.messages.push(Message {
            role: Role::User,
            content: user_input.to_string(),
        });

        self.truncate_context();

        self.run_loop(backend, None)
    }

    pub fn run_streaming(
        &mut self,
        backend: &mut dyn LlmBackend,
        user_input: &str,
        on_token: &mut dyn FnMut(&str),
    ) -> Result<String, String> {
        self.messages.push(Message {
            role: Role::User,
            content: user_input.to_string(),
        });

        self.truncate_context();

        self.run_loop(backend, Some(on_token))
    }

    fn run_loop(
        &mut self,
        backend: &mut dyn LlmBackend,
        mut on_token: Option<&mut dyn FnMut(&str)>,
    ) -> Result<String, String> {
        for _ in 0..self.max_turns {
            self.policy.start_turn();

            let response = if let Some(ref mut callback) = on_token {
                backend.complete_streaming(&self.messages, callback)?
            } else {
                backend.complete(&self.messages)?
            };

            if response.trim().is_empty() {
                return Ok(String::new());
            }

            let blocks = extract_alisp_blocks(&response);

            if blocks.is_empty() {
                self.messages.push(Message {
                    role: Role::Assistant,
                    content: response.clone(),
                });
                return Ok(response);
            }

            self.messages.push(Message {
                role: Role::Assistant,
                content: response.clone(),
            });

            let mut tool_output = String::new();
            for code in &blocks {
                let result = self.tools.execute(code);
                let output = match result {
                    Ok(val) => val,
                    Err(e) => format!("error: {}", e),
                };
                let output = self.policy.check_output(&output);
                tool_output.push_str(&format!("```\n{}\n```\n", output));
            }

            self.messages.push(Message {
                role: Role::Tool,
                content: tool_output,
            });
        }

        Err("max turns exceeded".to_string())
    }

    #[allow(dead_code)]
    pub fn clear_history(&mut self) {
        self.messages.truncate(1);
    }
}

fn extract_alisp_blocks(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut remaining = text;

    while let Some(start) = remaining.find("```alisp") {
        let after_tag = start + 8;
        if let Some(end) = remaining[after_tag..].find("```") {
            let code = remaining[after_tag..after_tag + end].trim().to_string();
            if !code.is_empty() {
                blocks.push(code);
            }
            remaining = &remaining[after_tag + end + 3..];
        } else {
            break;
        }
    }

    blocks
}
