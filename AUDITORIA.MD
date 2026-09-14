Auditoria Minuciosa de Código & Correções - Versão [Unreleased]
Realizamos uma auditoria minuciosa e aprofundada em todo o código das funcionalidades adicionadas na versão de desenvolvimento [Unreleased]:

Modo Dual-Pane Local ⇄ Remoto para Sessões SSH (TODO 5.7)
SSH Health Check & Latency RTT Probing (TODO 2.3)
Caderno de Bordo / Runbook com Narrativa Operacional (TODO 1.9)
Dashboard de Métricas de Recursos em Tempo Real (TODO 2.7)
Indicador de Execução em Andamento com Spinner e Timer (Live Running Feedback)
🔍 Problemas Identificados e Correções Aplicadas
1. Incompatibilidade Crítica de Assinatura e Retorno no Diff Remoto
Arquivos: 
src/onyxsh/filemanager/operations.py
 e 
src/onyxsh/filemanager/dual_pane.py
Problema:
Em RemoteDiffHelper.compare_files_async, o método operations.download_file_sync(...) era chamado passando o argumento session_override=session_item. No entanto, download_file_sync não aceitava o parâmetro session_override, resultando em um TypeError imediato ao tentar realizar a comparação de diffs.
Além disso, download_file_sync retornava uma tupla Tuple[bool, str], mas RemoteDiffHelper tratava o retorno como um booleano simples (if not success:). Como qualquer tupla não vazia ((False, "error")) avalia como True em Python, erros de download eram silenciosamente interpretados como sucesso.
No operations.py, download_file_sync montava o comando SCP usando a porta padrão 22 e chaves default, ignorando portas customizadas (session.port) e chave SSH privada configurada na sessão (session.key_path).
Correção:
download_file_sync em 
operations.py
 foi atualizado para aceitar session_override: Optional[SessionItem] = None, respeitando a porta remota (-P port para SCP) e a chave privada (-i key_path).
dual_pane.py
 foi ajustado para suportar com retrocompatibilidade retornos em tupla (bool, str) e booleanos legados, capturando mensagens de erro para o usuário.
2. Condição de Corrida na Navegação Rápida e Limpeza de Memória no Painel Local
Arquivos: 
src/onyxsh/filemanager/local_pane.py
 e 
src/onyxsh/filemanager/manager.py
Problema:
Se o usuário clicasse rapidamente em múltiplos diretórios ou breadcrumbs, a flag self._is_loading = True descartava silenciosamente as requisições subsequentes de navegação, deixando a interface em um diretório desatualizado.
O callback assíncrono _populate_store populava o Gio.ListStore sem validar se a pasta que acabou de ser lida correspondia à pasta atualmente solicitada (self.current_path), podendo exibir o conteúdo da pasta anterior caso as threads terminassem fora de ordem.
O componente LocalFileBrowserPane não possuía método destroy(), mantendo referências do Gtk.ColumnView e do modelo de seleção em memória.
Em PyGObject, Gio.ListStore implementa __len__, de modo que um store vazio (0 itens) avalia como False em if self.store:, impedindo a desconexão se não fosse feita a verificação explícita com is not None.
Correção:
Implementado contador de requisições (self._request_counter: int = 0). Cada navegação incrementa o ID e apenas respostas com req_id == self._request_counter e target_path == self.current_path são aplicadas.
Implementado o método destroy() em 
LocalFileBrowserPane
 desacoplando o modelo do Gtk.ColumnView, limpando os itens do Gio.ListStore e esvaziando caches.
Atualizado 
FileManager.destroy()
 para chamar explicitamente local_pane.destroy().
3. Sincronização Dinâmica de Configurações no SSHHealthMonitor
Arquivo: 
src/onyxsh/terminal/ssh_health_monitor.py
Problema:
SSHHealthMonitor lia as configurações apenas na inicialização. Se o usuário alterasse o intervalo de verificação, desativasse o health check ou alterasse parâmetros de auto-reconexão nas Preferências, o monitor continuava rodando com as configurações antigas até o aplicativo ser reiniciado.
Correção:
Registrado ouvinte _on_setting_changed via SettingsManager.add_change_listener, atualizando o temporizador do GLib dinamicamente em tempo real quando as chaves ssh_health_* ou ssh_auto_reconnect_* mudam.
Adicionada remoção do ouvinte no método stop().
4. Vazamento de Sinal Global no SSHErrorBanner
Arquivo: 
src/onyxsh/ui/widgets/ssh_error_banner.py
Problema:
Ao criar um SSHErrorBanner, o widget se conectava ao sinal global ssh-health-updated de AppSignals.get(), mas nunca desconectava o handler quando o banner era fechado, dispensado ou substituído por uma nova tentativa de conexão.
Isso retinha as instâncias do banner e de seus widgets filhos em memória, acumulando chamadas inúteis de atualização de contagem regressiva.
Correção:
Armazenado o _health_signal_id e implementado método destroy() que executa AppSignals.get().disconnect(self._health_signal_id).
Atualizado SSHErrorBannerManager.remove_banner() para chamar banner.destroy().
5. Robustez de Tratamento de Erro no Diálogo de Exportação (FileDialog)
Arquivo: 
src/onyxsh/ui/dialogs/export_dialog.py
Problema:
A verificação de cancelamento do usuário em _on_file_dialog_saved utilizava err.code != Gtk.DialogError.DISMISSED. Em certos ambientes PyGObject GTK4, acessar diretamente err.code pode falhar dependendo da versão dos bindings.
Correção:
Utilizado o método oficial PyGObject not err.matches(Gtk.DialogError.quark(), Gtk.DialogError.DISMISSED) com fallback de exceção geral except Exception as err:.
6. Proteção Defensiva no Indicador de Execução do Terminal
Arquivo: 
src/onyxsh/terminal/tabs.py
Problema:
Se invocado diretamente, show_command_running_indicator não conferia se a configuração "show_command_running_indicator" estava desativada pelo usuário.
Correção:
Adicionada checagem defensiva nas propriedades do SettingsManager antes de iniciar o timer periódico de 100ms.
🧪 Validação com Testes Automatizados
Foram adicionados novos testes unitários cobrindo todos os cenários corrigidos:

test_settings_change_listener em 
tests/test_ssh_health_monitor.py
test_error_banner_signal_cleanup em 
tests/test_ssh_health_monitor.py
test_local_pane_destroy_and_stale_request_protection em 
tests/test_filemanager_dual_pane.py
test_download_file_sync_session_override_and_port em 
tests/test_filemanager_dual_pane.py
Execução da Suíte Completa de Testes:
bash

PYTHONPATH=src python3 -m unittest discover -s tests
Resultado:


Ran 499 tests in 14.167s
OK (100% de sucesso, 0 falhas, 0 erros, 0 regressões)
