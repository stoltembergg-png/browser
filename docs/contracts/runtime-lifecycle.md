# Runtime lifecycle contract

Este contrato é pré-requisito para PR-020, PR-023, PR-025, PR-028, PR-036 e PR-040–043. Ele transforma lifecycle, cancelamento, backpressure e recovery em estados verificáveis.

## 1. Owners e fencing

- O `browser-core` é o único owner das state machines de app/session/profile/tab/request.
- O engine host é o único owner da instância Servo e de seus objetos thread-affine.
- Cada engine instance, tab, navigation e request possui uma generation/epoch monotônica.
- Fechar ou substituir uma entidade incrementa a generation antes de emitir efeitos externos; qualquer resposta antiga falha como `StaleContext`.
- Cada command e cada request de popup/permission/download produz exatamente um resultado terminal: `Completed`, `Failed`, `Cancelled`, `TimedOut`, `Closed` ou `Crashed`.
- Uma segunda decisão para o mesmo request retorna `AlreadyResolved`; não reaplica efeitos.

## 2. Estados e transições normativas

Estados ativos, recuperáveis e terminais são distintos. A tabela de transições abaixo é fechada: qualquer source→target não listado falha com `InvalidTransition` e não produz efeito externo.

| Entidade | Estados ativos | Estados terminais |
|---|---|---|
| App | `Starting`, `Running`, `Quiescing` | `Closed`, `Failed` |
| Session/profile | `Opening`, `Open`, `Migrating`, `Closing` | `Closed`, `Corrupt` |
| Engine instance | `Created`, `Starting`, `Ready`, `Stopping`, `Crashed`, `Restarting` | `Exited`, `Failed` |
| Tab/webview | `Creating`, `Ready`, `Navigating`, `Suspended`, `Closing`, `Crashed`, `Restarting` | `Closed`, `Failed` |
| Request | `Pending`, `Prompted`, `Executing` | `Completed`, `Failed`, `Cancelled`, `TimedOut`, `Closed`, `Crashed` |

### 2.1 Matriz source→target

| Entidade | Source → target | Ator/guarda | Efeito/evento e idempotência |
|---|---|---|---|
| App | `Starting → Running` | bootstrap completo e profile aberto | emite `AppReady`; repetição retorna estado atual |
| App | `Starting → Failed` | startup error irrecuperável | emite erro terminal; não cria janela parcial |
| App | `Running → Quiescing` | close/shutdown aceito pelo core | incrementa epoch de app e rejeita novas mutações |
| App | `Quiescing → Closed` | snapshot/repositories/engine barrier concluídos | emite `AppClosed`; repetição é no-op |
| Session | `Opening → Open` | lock e schema válidos | emite `ProfileOpened`; lock é adquirido uma vez |
| Session | `Opening → Corrupt` | checksum/journal/backup inválido | quarentena; nenhuma escrita destrutiva |
| Session | `Open → Migrating` | schema antigo e migration autorizada | grava migration marker; retry usa o mesmo step |
| Session | `Migrating → Open` | commit/checksum válidos | publica nova schema revision uma vez |
| Session | `Migrating → Corrupt` | interrupção sem recovery seguro | preserva backup/last-known-good; bloqueia startup |
| Session | `Open → Closing → Closed` | profile close após writes drenados | flush atômico e libera lock; segunda close retorna estado atual |
| Engine | `Created → Starting → Ready` | host owner e surface aprovados | `EngineStarted/Ready`; create é idempotente por instance ID |
| Engine | `Starting → Failed` | builder/waker/surface falha | nenhum objeto parcial é exposto |
| Engine | `Ready → Stopping → Exited` | close/shutdown barrier | drena reliable-control e libera objetos thread-affine uma vez |
| Engine | `Ready → Crashed → Restarting → Ready` | crash e retry permitido | nova instance/epoch; eventos antigos tornam-se stale |
| Engine | `Crashed → Exited` | retry proibido ou esgotado | emite `EngineLost`; não reinicia silenciosamente |
| Tab | `Creating → Ready` | engine webview criado | publica tab handle; duplicata retorna `AlreadyExists` |
| Tab | `Ready → Navigating` | navigation intent autorizado | incrementa navigation generation |
| Tab | `Navigating → Ready` | commit/finish ainda vigente | commit acontece somente após fence check |
| Tab | `Ready/Navigating → Closing → Closed` | close vence antes de novo efeito | invalida generation; late events são `StaleContext` |
| Tab | `Ready/Navigating → Crashed → Restarting` | engine loss e policy segura | restaura URL segura/blank; não reenvia formulário |
| Request | `Pending → Prompted → Executing` | owner/context/capability válidos | correlation permanece única; expiry cancela prompt |
| Request | `Pending/Prompted/Executing → Completed` | efeito reservado e commit bem-sucedido | exatamente um terminal; decisão repetida é `AlreadyResolved` |
| Request | `Pending/Prompted/Executing → Failed/Cancelled/TimedOut/Closed/Crashed` | guard correspondente antes do commit | erro/terminal explícito; nenhum efeito tardio |

Toda transição documenta ator, guarda, efeito, evento, deadline e idempotência. O reducer single-writer do `browser-core` é o único ponto de linearização: ele verifica o fence composto `(engine_instance_id, tab_id, webview_id, navigation_id, request_id, epoch)` e reserva o commit atomicamente antes de qualquer efeito externo.

Precedência: `Close/Shutdown` invalida a geração antes de aceitar novos efeitos; `Cancelled`/`TimedOut` vence somente se o commit ainda não foi reservado; depois da reserva, `Completed` ou `Failed` vence e o cancelamento retorna `AlreadyResolved`; qualquer evento de geração anterior é descartado. Não existe “último callback vence”.

### 2.2 Ownership de loops e shutdown barrier

| Recurso | Owner único | Wake/entrada permitida | Regra de encerramento |
|---|---|---|---|
| Tauri/window loop | shell Tauri | eventos nativos e intents tipados; nunca `spin_event_loop` prolongado | para aceitar novos intents, aguarda snapshot final e destrói janelas somente após `Closed` |
| Browser-core actor | `browser-core` | mailbox serializada de commands e callbacks normalizados | torna-se `Quiescing`, rejeita novas mutações e emite terminais antes de fechar repositories |
| Servo event loop | engine host thread-affine | `EventLoopWaker` com wake coalescing; somente o host chama `spin_event_loop` | drena comandos reliable, cancela realtime, aguarda callback/frame barrier e só então libera Servo objects |
| Tokio/task runtime | orchestration owner definido pelo core | timers, IO e bounded tasks; sem acesso direto a tipos Servo | cancela por hierarquia, aguarda join/timeout e transforma tasks órfãs em diagnóstico bloqueante |
| RenderingContext/surface | engine host + render-surface | `paint/present` somente no owner da surface | não aceita `present` após `Closing`; frame pendente vira `StaleContext`, nunca callback após drop |

Wakeups tardios, callbacks depois do drop, frame pendente durante fechamento, renderer lento e tempestade de 10.000 inputs/navegações são casos negativos obrigatórios. Deadlock watchdog, fila ilimitada ou perda de evento terminal são `NO_GO`.

## 3. Interleavings obrigatórios

O test harness deve executar, de modo determinístico e também model-based:

- `Stop × NavigationCommitted`;
- `CloseTab × PermissionResolve`;
- `CloseTab × PopupCreate`;
- `CancelDownload × FinalizeRename`;
- `Crash × Restart × LateEvent`;
- `Timeout × Completion`;
- `ProfileClose × MigrationCommit`;
- `Shutdown × NewCommand`;
- duplicate command/decision and replayed event.

O resultado esperado é uma única transição terminal, sem resurrect, double commit, grant tardio, arquivo final órfão ou reenvio de formulário.

## 4. Channel contract

| Classe | Exemplos | Entrega | Saturação |
|---|---|---|---|
| Realtime/coalescible | input, resize, frame hint | bounded, latest-wins por tab/viewport | coalesce explícito e metricado |
| Reliable control | navigation, stop, close, permission, download decision | lane lossless ou admission reject | erro/ack determinístico; nunca drop silencioso |
| Diagnostics | tracing, progress auxiliar | best effort | drop permitido com contador |

Capacidades, quotas, fairness, reserva de slots para `Reliable control`, timeout, drain e cancelamento devem estar em um manifest de runtime ratificado pelo ADR-005. A ausência desse manifest bloqueia PR-023/025; não se escolhe um número arbitrário no documento.

## 5. Recovery matrix

Cada linha é um boundary de efeito externo. O artifact de recovery deve registrar o kill point, checkpoint/journal, abort seguro, resultado terminal, retry permitido e fence da nova engine; ausência de qualquer coluna é `NO_GO`.

| Operação/boundary | Kill point | Checkpoint/journal | Abort seguro | Resultado terminal | Retry/rollback | Fence pós-restart |
|---|---|---|---|---|---|---|
| Engine create | após alocação, antes de `Ready` | instance reservation | destruir surface/handles parciais | `Failed` ou `Crashed` | retry só com nova instance | `engine_instance_id` novo |
| Navigation/load | URL aceita, resposta recebida, document committed | navigation intent + generation | descartar documento/stream não committed | `Cancelled`, `Failed` ou `Crashed` | somente GET idempotente e policy explícita | navigation generation nova |
| Frame/present | frame pintado antes/depois de present | frame sequence + surface epoch | não apresentar surface fechada | `StaleContext`/`Failed` | nunca reapresentar frame stale | surface/engine epoch nova |
| Input/resize | evento enfileirado ou aplicado parcialmente | input sequence/viewport revision | coalescer somente realtime vigente | `Completed` ou `StaleContext` | replay só de resize/input seguro | tab/webview generation |
| Tab/engine close | antes/depois de shutdown barrier | quiesce marker + drain state | liberar objetos thread-affine na ordem owner | `Closed` ou `Crashed` | restart de tab segura; não reabrir fechada | tab + engine epoch novos |
| Download | temp criado, chunk recebido, rename final | temp marker + byte/checksum journal | remover/quarentenar temp incompleto | `Cancelled`, `Failed` ou `Completed` | nunca finalizar duas vezes | request/download generation |
| Permission/popup | prompt aberto ou decisão reservada | request correlation + grant marker | fechar request sem grant tardio | `Cancelled`, `TimedOut` ou `Completed` | não reaplicar grant/criar janela | opener/target/request epoch |
| Session write | antes/depois de flush | journal + commit marker | manter snapshot anterior e orphan temp | `Failed` ou `Completed` | retry transacional | profile/session revision |
| Migration | antes/depois de schema commit | schema journal/checksum + backup | restaurar backup íntegro, bloquear startup | `Corrupt` ou `Completed` | somente protocolo forward-safe | schema/profile revision |

Crashes e kills devem ser injetados em cada boundary acima. Cada ponto exige checkpoint/abort seguro, retry explícito, fencing da geração antiga e replay sem duplicação após restart; `partial write`, orphan temp, frame tardio e evento de engine antiga devem produzir diagnóstico verificável, não sucesso sintético.

## 6. Evidence

Cada teste de lifecycle publica interleaving manifest, seed/model revision, command sequence, expected terminal state, actual event log, repository/head/tree SHA, engine revision e artifact digest. Zero casos executados, caso skipped ou fixture ausente é `NO_GO` salvo por applicability policy versionada e verificada.
