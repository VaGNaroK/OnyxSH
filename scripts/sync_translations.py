#!/usr/bin/env python3
"""
Zashterminal Translation Synchronization Tool
Synchronizes newly added msgid entries across all 28 supported .po files in locale/
and compiles them into .mo catalog files.
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path
from typing import Dict, List, Tuple

REPO_ROOT = Path(__file__).resolve().parent.parent
LOCALE_DIR = REPO_ROOT / "locale"
INTERNAL_LOCALE_DIR = REPO_ROOT / "src" / "zashterminal" / "locale"

# Catalog of newly added strings with multilingual translations
NEW_TRANSLATIONS: Dict[str, Dict[str, str]] = {
    "Export conversation": {
        "pt": "Exportar conversa",
        "en": "Export conversation",
        "es": "Exportar conversación",
        "fr": "Exporter la conversation",
        "de": "Unterhaltung exportieren",
        "it": "Esporta conversazione",
        "zh": "导出对话",
        "ja": "会話をエクスポート",
        "ru": "Экспорт диалога",
    },
    "Export as Markdown (.md)": {
        "pt": "Exportar como Markdown (.md)",
        "en": "Export as Markdown (.md)",
        "es": "Exportar como Markdown (.md)",
        "fr": "Exporter en Markdown (.md)",
        "de": "Als Markdown (.md) exportieren",
        "it": "Esporta come Markdown (.md)",
        "zh": "导出为 Markdown (.md)",
        "ja": "Markdown (.md) としてエクスポート",
        "ru": "Экспортировать как Markdown (.md)",
    },
    "Export as JSON (.json)": {
        "pt": "Exportar como JSON (.json)",
        "en": "Export as JSON (.json)",
        "es": "Exportar como JSON (.json)",
        "fr": "Exporter en JSON (.json)",
        "de": "Als JSON (.json) exportieren",
        "it": "Esporta come JSON (.json)",
        "zh": "导出为 JSON (.json)",
        "ja": "JSON (.json) としてエクスポート",
        "ru": "Экспортировать как JSON (.json)",
    },
    "Copy to Clipboard": {
        "pt": "Copiar para a área de transferência",
        "en": "Copy to Clipboard",
        "es": "Copiar al portapapeles",
        "fr": "Copier dans le presse-papier",
        "de": "In die Zwischenablage kopieren",
        "it": "Copia negli appunti",
        "zh": "复制到剪贴板",
        "ja": "クリップボードにコピー",
        "ru": "Скопировать в буфер обмена",
    },
    "No messages to export.": {
        "pt": "Nenhuma mensagem para exportar.",
        "en": "No messages to export.",
        "es": "No hay mensajes para exportar.",
        "fr": "Aucun message à exporter.",
        "de": "Keine Nachrichten zum Exportieren vorhanden.",
        "it": "Nessun messaggio da esportare.",
        "zh": "没有可导出的消息。",
        "ja": "エクスポートするメッセージがありません。",
        "ru": "Нет сообщений для экспорта.",
    },
    "Conversation copied to clipboard.": {
        "pt": "Conversa copiada para a área de transferência.",
        "en": "Conversation copied to clipboard.",
        "es": "Conversación copiada al portapapeles.",
        "fr": "Conversation copiée dans le presse-papier.",
        "de": "Unterhaltung in die Zwischenablage kopiert.",
        "it": "Conversazione copiata negli appunti.",
        "zh": "对话已复制到剪贴板。",
        "ja": "会話をクリップボードにコピーしました。",
        "ru": "Диалог скопирован в буфер обмена.",
    },
    "Export AI Conversation": {
        "pt": "Exportar conversa da IA",
        "en": "Export AI Conversation",
        "es": "Exportar conversación de IA",
        "fr": "Exporter la conversation IA",
        "de": "KI-Unterhaltung exportieren",
        "it": "Esporta conversazione IA",
        "zh": "导出 AI 对话",
        "ja": "AI 会話をエクスポート",
        "ru": "Экспорт диалога ИИ",
    },
    "JSON files (*.json)": {
        "pt": "Arquivos JSON (*.json)",
        "en": "JSON files (*.json)",
        "es": "Archivos JSON (*.json)",
        "fr": "Fichiers JSON (*.json)",
        "de": "JSON-Dateien (*.json)",
        "it": "File JSON (*.json)",
        "zh": "JSON 文件 (*.json)",
        "ja": "JSON ファイル (*.json)",
        "ru": "Файлы JSON (*.json)",
    },
    "Markdown files (*.md)": {
        "pt": "Arquivos Markdown (*.md)",
        "en": "Markdown files (*.md)",
        "es": "Archivos Markdown (*.md)",
        "fr": "Fichiers Markdown (*.md)",
        "de": "Markdown-Dateien (*.md)",
        "it": "File Markdown (*.md)",
        "zh": "Markdown 文件 (*.md)",
        "ja": "Markdown ファイル (*.md)",
        "ru": "Файлы Markdown (*.md)",
    },
    "All files": {
        "pt": "Todos os arquivos",
        "en": "All files",
        "es": "Todos los archivos",
        "fr": "Tous les fichiers",
        "de": "Alle Dateien",
        "it": "Tutti i file",
        "zh": "所有文件",
        "ja": "すべてのファイル",
        "ru": "Все файлы",
    },
    "Conversation exported successfully.": {
        "pt": "Conversa exportada com sucesso.",
        "en": "Conversation exported successfully.",
        "es": "Conversación exportada con éxito.",
        "fr": "Conversation exportée avec succès.",
        "de": "Unterhaltung erfolgreich exportiert.",
        "it": "Conversazione esportata con successo.",
        "zh": "对话导出成功。",
        "ja": "会話が正常にエクスポートされました。",
        "ru": "Диалог успешно экспортирован.",
    },
    "Failed to export conversation: {}": {
        "pt": "Falha ao exportar conversa: {}",
        "en": "Failed to export conversation: {}",
        "es": "Error al exportar conversación: {}",
        "fr": "Échec de l'exportation de la conversation : {}",
        "de": "Fehler beim Exportieren der Unterhaltung: {}",
        "it": "Impossibile esportare la conversazione: {}",
        "zh": "导出对话失败：{}",
        "ja": "会話のエクスポートに失敗しました: {}",
        "ru": "Не удалось экспортировать диалог: {}",
    },
    "Paleta de Comandos": {
        "pt": "Paleta de Comandos",
        "en": "Command Palette",
        "es": "Paleta de Comandos",
        "fr": "Palette de commandes",
        "de": "Befehlspalette",
        "it": "Tavolozza dei comandi",
        "zh": "命令面板",
        "ja": "コマンドパレット",
        "ru": "Палитра команд",
    },
    "Command Palette": {
        "pt": "Paleta de Comandos",
        "en": "Command Palette",
        "es": "Paleta de Comandos",
        "fr": "Palette de commandes",
        "de": "Befehlspalette",
        "it": "Tavolozza dei comandi",
        "zh": "命令面板",
        "ja": "コマンドパレット",
        "ru": "Палитра команд",
    },
    "Digite um comando ou busque uma ação...": {
        "pt": "Digite um comando ou busque uma ação...",
        "en": "Type a command or search an action...",
        "es": "Escriba un comando o busque una acción...",
        "fr": "Tapez une commande ou recherchez une action...",
        "de": "Befehl eingeben oder nach Aktion suchen...",
        "it": "Digita un comando o cerca un'azione...",
        "zh": "输入命令或搜索操作...",
        "ja": "コマンドを入力するかアクションを検索...",
        "ru": "Введите команду или найдите действие...",
    },
    "Abas e Janelas": {
        "pt": "Abas e Janelas",
        "en": "Tabs & Windows",
        "es": "Pestañas y Ventanas",
        "fr": "Onglets et fenêtres",
        "de": "Tabs & Fenster",
        "it": "Schede e finestre",
        "zh": "标签与窗口",
        "ja": "タブとウィンドウ",
        "ru": "Вкладки и окна",
    },
    "Divisão de Telas": {
        "pt": "Divisão de Telas",
        "en": "Screen Splitting",
        "es": "División de Pantallas",
        "fr": "Division d'écran",
        "de": "Bildschirmteilung",
        "it": "Divisione schermo",
        "zh": "屏幕分屏",
        "ja": "画面分割",
        "ru": "Разделение экрана",
    },
    "Sessões & SSH": {
        "pt": "Sessões & SSH",
        "en": "Sessions & SSH",
        "es": "Sesiones y SSH",
        "fr": "Sessions et SSH",
        "de": "Sitzungen & SSH",
        "it": "Sessioni e SSH",
        "zh": "会话与 SSH",
        "ja": "セッションと SSH",
        "ru": "Сессии и SSH",
    },
    "Fechar Aba Atual": {
        "pt": "Fechar Aba Atual",
        "en": "Close Current Tab",
        "es": "Cerrar Pestaña Actual",
        "fr": "Fermer l'onglet actuel",
        "de": "Aktuellen Tab schließen",
        "it": "Chiudi scheda corrente",
        "zh": "关闭当前标签",
        "ja": "現在のタブを閉じる",
        "ru": "Закрыть текущую вкладку",
    },
    "Aba Anterior": {
        "pt": "Aba Anterior",
        "en": "Previous Tab",
        "es": "Pestaña Anterior",
        "fr": "Onglet précédent",
        "de": "Vorheriger Tab",
        "it": "Scheda precedente",
        "zh": "上一标签",
        "ja": "前のタブ",
        "ru": "Предыдущая вкладка",
    },
    "Dividir Painel Horizontalmente": {
        "pt": "Dividir Painel Horizontalmente",
        "en": "Split Pane Horizontally",
        "es": "Dividir Panel Horizontalmente",
        "fr": "Diviser le panneau horizontalement",
        "de": "Bereich horizontal teilen",
        "it": "Dividi riquadro orizzontalmente",
        "zh": "水平分屏",
        "ja": "ペインを水平分割",
        "ru": "Разделить панель горизонтально",
    },
    "Dividir Painel Verticalmente": {
        "pt": "Dividir Painel Verticalmente",
        "en": "Split Pane Vertically",
        "es": "Dividir Panel Verticalmente",
        "fr": "Diviser le panneau verticalement",
        "de": "Bereich vertikal teilen",
        "it": "Dividi riquadro verticalmente",
        "zh": "垂直分屏",
        "ja": "ペインを垂直分割",
        "ru": "Разделить панель вертикально",
    },
    "Fechar Painel Dividido": {
        "pt": "Fechar Painel Dividido",
        "en": "Close Split Pane",
        "es": "Cerrar Panel Dividido",
        "fr": "Fermer le panneau divisé",
        "de": "Geteilten Bereich schließen",
        "it": "Chiudi riquadro diviso",
        "zh": "关闭分屏",
        "ja": "分割ペインを閉じる",
        "ru": "Закрыть разделенную панель",
    },
    "Focar Painel Acima": {
        "pt": "Focar Painel Acima",
        "en": "Focus Pane Above",
        "es": "Enfocar Panel Superior",
        "fr": "Focus sur le panneau au-dessus",
        "de": "Oberen Bereich fokussieren",
        "it": "Sposta messa a fuoco sul riquadro superiore",
        "zh": "聚焦上方分屏",
        "ja": "上のペインにフォーカス",
        "ru": "Фокус на верхней панели",
    },
    "Focar Painel Abaixo": {
        "pt": "Focar Painel Abaixo",
        "en": "Focus Pane Below",
        "es": "Enfocar Panel Inferior",
        "fr": "Focus sur le panneau en-dessous",
        "de": "Unteren Bereich fokussieren",
        "it": "Sposta messa a fuoco sul riquadro inferiore",
        "zh": "聚焦下方分屏",
        "ja": "下のペインにフォーカス",
        "ru": "Фокус на нижней панели",
    },
    "Focar Painel à Esquerda": {
        "pt": "Focar Painel à Esquerda",
        "en": "Focus Pane to the Left",
        "es": "Enfocar Panel a la Izquierda",
        "fr": "Focus sur le panneau à gauche",
        "de": "Linken Bereich fokussieren",
        "it": "Sposta messa a fuoco sul riquadro a sinistra",
        "zh": "聚焦左侧分屏",
        "ja": "左のペインにフォーカス",
        "ru": "Фокус на левой панели",
    },
    "Focar Painel à Direita": {
        "pt": "Focar Painel à Direita",
        "en": "Focus Pane to the Right",
        "es": "Enfocar Panel a la Derecha",
        "fr": "Focus sur le panneau à droite",
        "de": "Rechten Bereich fokussieren",
        "it": "Sposta messa a fuoco sul riquadro a destra",
        "zh": "聚焦右侧分屏",
        "ja": "右のペインにフォーカス",
        "ru": "Фокус на правой панели",
    },
    "Copiar Seleção": {
        "pt": "Copiar Seleção",
        "en": "Copy Selection",
        "es": "Copiar Selección",
        "fr": "Copier la sélection",
        "de": "Auswahl kopieren",
        "it": "Copia selezione",
        "zh": "复制所选内容",
        "ja": "選択項目をコピー",
        "ru": "Скопировать выделенное",
    },
    "Colar Área de Transferência": {
        "pt": "Colar Área de Transferência",
        "en": "Paste from Clipboard",
        "es": "Pegar desde el Portapapeles",
        "fr": "Coller depuis le presse-papier",
        "de": "Aus Zwischenablage einfügen",
        "it": "Incolla dagli appunti",
        "zh": "从剪贴板粘贴",
        "ja": "クリップボードから貼り付け",
        "ru": "Вставить из буфера обмена",
    },
    "Limpar Tela do Terminal": {
        "pt": "Limpar Tela do Terminal",
        "en": "Clear Terminal Screen",
        "es": "Limpiar Pantalla del Terminal",
        "fr": "Effacer l'écran du terminal",
        "de": "Terminalbildschirm löschen",
        "it": "Pulisci schermo del terminale",
        "zh": "清除终端屏幕",
        "ja": "ターミナル画面をクリア",
        "ru": "Очистить экран терминала",
    },
    "Buscar no Terminal": {
        "pt": "Buscar no Terminal",
        "en": "Search in Terminal",
        "es": "Buscar en el Terminal",
        "fr": "Rechercher dans le terminal",
        "de": "Im Terminal suchen",
        "it": "Cerca nel terminale",
        "zh": "在终端中搜索",
        "ja": "ターミナル内を検索",
        "ru": "Поиск в терминале",
    },
    "Modo Transmissão (Comando p/ Todas as Abas)": {
        "pt": "Modo Transmissão (Comando p/ Todas as Abas)",
        "en": "Broadcast Mode (Command to All Tabs)",
        "es": "Modo Transmisión (Comando para Todas las Pestañas)",
        "fr": "Mode diffusion (commande vers tous les onglets)",
        "de": "Broadcast-Modus (Befehl an alle Tabs senden)",
        "it": "Modalità trasmissione (comando per tutte le schede)",
        "zh": "广播模式 (发送命令至所有标签)",
        "ja": "ブロードキャストモード (すべてのタブにコマンド送信)",
        "ru": "Режим трансляции (команда на все вкладки)",
    },
    "Aumentar Zoom / Tamanho da Fonte": {
        "pt": "Aumentar Zoom / Tamanho da Fonte",
        "en": "Zoom In / Increase Font Size",
        "es": "Aumentar Zoom / Tamaño de Fuente",
        "fr": "Zoom avant / Augmenter la police",
        "de": "Vergrößern / Schriftgröße erhöhen",
        "it": "Ingrandisci / Aumenta dimensione carattere",
        "zh": "放大 / 增大字体",
        "ja": "ズームイン / フォント拡大",
        "ru": "Увеличить масштаб / шрифт",
    },
    "Diminuir Zoom / Tamanho da Fonte": {
        "pt": "Diminuir Zoom / Tamanho da Fonte",
        "en": "Zoom Out / Decrease Font Size",
        "es": "Disminuir Zoom / Tamaño de Fuente",
        "fr": "Zoom arrière / Diminuer la police",
        "de": "Verkleinern / Schriftgröße verringern",
        "it": "Rimpicciolisci / Diminuisci dimensione carattere",
        "zh": "缩小 / 减小字体",
        "ja": "ズームアウト / フォント縮小",
        "ru": "Уменьшить масштаб / шрифт",
    },
    "Resetar Zoom": {
        "pt": "Resetar Zoom",
        "en": "Reset Zoom",
        "es": "Restablecer Zoom",
        "fr": "Réinitialiser le zoom",
        "de": "Zoom zurücksetzen",
        "it": "Ripristina zoom",
        "zh": "重置缩放",
        "ja": "ズームをリセット",
        "ru": "Сбросить масштаб",
    },
    "Assistente de IA (Abrir / Fechar)": {
        "pt": "Assistente de IA (Abrir / Fechar)",
        "en": "AI Assistant (Open / Close)",
        "es": "Asistente de IA (Abrir / Cerrar)",
        "fr": "Assistant IA (Ouvrir / Fermer)",
        "de": "KI-Assistent (Öffnen / Schließen)",
        "it": "Assistente IA (Apri / Chiudi)",
        "zh": "AI 助手 (打开 / 关闭)",
        "ja": "AI アシスタント (開く / 閉じる)",
        "ru": "ИИ-помощник (Открыть / Закрыть)",
    },
    "Configurar Provedores e Modelos de IA": {
        "pt": "Configurar Provedores e Modelos de IA",
        "en": "Configure AI Providers & Models",
        "es": "Configurar Proveedores y Modelos de IA",
        "fr": "Configurer les fournisseurs et modèles d'IA",
        "de": "KI-Anbieter und -Modelle konfigurieren",
        "it": "Configura provider e modelli IA",
        "zh": "配置 AI 提供商与模型",
        "ja": "AI プロバイダーとモデルの設定",
        "ru": "Настроить провайдеров и модели ИИ",
    },
    "Políticas de Segurança e Escopo do Agente": {
        "pt": "Políticas de Segurança e Escopo do Agente",
        "en": "Agent Security Policies & Scope",
        "es": "Políticas de Seguridad y Alcance del Agente",
        "fr": "Politiques de sécurité et portée de l'agent",
        "de": "Sicherheitsrichtlinien & Agentenbereich",
        "it": "Criteri di sicurezza e ambito dell'agente",
        "zh": "代理安全策略与作用域",
        "ja": "エージェントのセキュリティポリシーとスコープ",
        "ru": "Политики безопасности и область агента",
    },
    "Registro de Auditoria de Ações do Agente": {
        "pt": "Registro de Auditoria de Ações do Agente",
        "en": "Agent Action Audit Log",
        "es": "Registro de Auditoría de Acciones del Agente",
        "fr": "Journal d'audit des actions de l'agent",
        "de": "Aktionsüberwachungsprotokoll des Agenten",
        "it": "Registro di controllo delle azioni dell'agente",
        "zh": "代理操作审计日志",
        "ja": "エージェントアクション監査ログ",
        "ru": "Журнал аудита действий агента",
    },
    "Painel Lateral de Sessões SSH": {
        "pt": "Painel Lateral de Sessões SSH",
        "en": "SSH Sessions Sidebar",
        "es": "Panel Lateral de Sesiones SSH",
        "fr": "Panneau latéral des sessions SSH",
        "de": "SSH-Sitzungsseitenleiste",
        "it": "Barra laterale delle sessioni SSH",
        "zh": "SSH 会话侧边栏",
        "ja": "SSH セッションサイドバー",
        "ru": "Боковая панель SSH-сессий",
    },
    "Gerenciador de Arquivos SFTP": {
        "pt": "Gerenciador de Arquivos SFTP",
        "en": "SFTP File Manager",
        "es": "Administrador de Archivos SFTP",
        "fr": "Gestionnaire de fichiers SFTP",
        "de": "SFTP-Dateimanager",
        "it": "Gestore file SFTP",
        "zh": "SFTP 文件管理器",
        "ja": "SFTP ファイルマネージャー",
        "ru": "Файловый менеджер SFTP",
    },
    "Gerenciador de Comandos e Snippets": {
        "pt": "Gerenciador de Comandos e Snippets",
        "en": "Command Manager & Snippets",
        "es": "Administrador de Comandos y Snippets",
        "fr": "Gestionnaire de commandes et extraits",
        "de": "Befehls-Manager & Snippets",
        "it": "Gestore comandi e frammenti",
        "zh": "命令管理器与代码片段",
        "ja": "コマンドマネージャーとスニペット",
        "ru": "Менеджер команд и сниппетов",
    },
    "Servidor TFTP Embutido": {
        "pt": "Servidor TFTP Embutido",
        "en": "Built-in TFTP Server",
        "es": "Servidor TFTP Integrado",
        "fr": "Serveur TFTP intégré",
        "de": "Integrierter TFTP-Server",
        "it": "Server TFTP integrato",
        "zh": "内置 TFTP 服务器",
        "ja": "組み込み TFTP サーバー",
        "ru": "Встроенный TFTP-сервер",
    },
    "Salvar Layout de Sessões Atual": {
        "pt": "Salvar Layout de Sessões Atual",
        "en": "Save Current Session Layout",
        "es": "Guardar Diseño de Sesiones Actual",
        "fr": "Enregistrer la disposition des sessions",
        "de": "Aktuelles Sitzungslayout speichern",
        "it": "Salva layout sessione corrente",
        "zh": "保存当前会话布局",
        "ja": "現在のセッションレイアウトを保存",
        "ru": "Сохранить текущий макет сессий",
    },
    "Importar Sessões do SecureCRT": {
        "pt": "Importar Sessões do SecureCRT",
        "en": "Import SecureCRT Sessions",
        "es": "Importar Sesiones de SecureCRT",
        "fr": "Importer des sessions SecureCRT",
        "de": "SecureCRT-Sitzungen importieren",
        "it": "Importa sessioni SecureCRT",
        "zh": "导入 SecureCRT 会话",
        "ja": "SecureCRT セッションのインポート",
        "ru": "Импортировать сессии SecureCRT",
    },
    "Preferências do Zashterminal": {
        "pt": "Preferências do Zashterminal",
        "en": "Zashterminal Preferences",
        "es": "Preferencias de Zashterminal",
        "fr": "Préférences de Zashterminal",
        "de": "Zashterminal-Einstellungen",
        "it": "Preferenze di Zashterminal",
        "zh": "Zashterminal 首选项",
        "ja": "Zashterminal 設定",
        "ru": "Настройки Zashterminal",
    },
    "Atalhos de Teclado": {
        "pt": "Atalhos de Teclado",
        "en": "Keyboard Shortcuts",
        "es": "Atajos de Teclado",
        "fr": "Raccourcis clavier",
        "de": "Tastaturkürzel",
        "it": "Scorciatoie da tastiera",
        "zh": "快捷键",
        "ja": "キーボードショートカット",
        "ru": "Горячие клавиши",
    },
    "Regras de Realce Sintático e Temas": {
        "pt": "Regras de Realce Sintático e Temas",
        "en": "Syntax Highlighting Rules & Themes",
        "es": "Reglas de Resaltado de Sintaxis y Temas",
        "fr": "Règles de coloration syntaxique et thèmes",
        "de": "Syntaxhervorhebungsregeln & Themes",
        "it": "Regole di evidenziazione della sintassi e temi",
        "zh": "语法高亮规则与主题",
        "ja": "シンタックスハイライトのルールとテーマ",
        "ru": "Правила подсветки синтаксиса и темы",
    },
    "Conectar:": {
        "pt": "Conectar:",
        "en": "Connect:",
        "es": "Conectar:",
        "fr": "Connexion :",
        "de": "Verbinden:",
        "it": "Connetti:",
        "zh": "连接：",
        "ja": "接続:",
        "ru": "Подключить:",
    },
    "How to handle tabs and layout from previous session": {
        "pt": "Como gerenciar abas e layout da sessão anterior",
        "en": "How to handle tabs and layout from previous session",
        "es": "Cómo manejar las pestañas y el diseño de la sesión anterior",
        "fr": "Comment gérer les onglets et la disposition de la session précédente",
        "de": "Wie Tabs und Layout der vorherigen Sitzung gehandhabt werden",
        "it": "Come gestire le schede e il layout della sessione precedente",
        "zh": "如何处理上一会话的标签页和布局",
        "ja": "前回のセッションのタブとレイアウトの処理方法",
        "ru": "Как обрабатывать вкладки и макет предыдущей сессии",
    },
    "Auto-reconnect SSH sessions": {
        "pt": "Reconectar sessões SSH automaticamente",
        "en": "Auto-reconnect SSH sessions",
        "es": "Reconectar automáticamente sesiones SSH",
        "fr": "Reconnecter automatiquement les sessions SSH",
        "de": "SSH-Sitzungen automatisch wiederverbinden",
        "it": "Riconnetti automaticamente le sessioni SSH",
        "zh": "自动重新连接 SSH 会话",
        "ja": "SSH セッションを自動再接続",
        "ru": "Автоматически переподключать сессии SSH",
    },
    "Automatically re-establish open remote connections": {
        "pt": "Tenta restabelecer conexões remotas que estavam abertas",
        "en": "Automatically re-establish open remote connections",
        "es": "Restablece automáticamente las conexiones remotas abiertas",
        "fr": "Rétablit automatiquement les connexions distantes ouvertes",
        "de": "Offene Remote-Verbindungen automatisch wiederherstellen",
        "it": "Ristabilisce automaticamente le connessioni remote aperte",
        "zh": "自动重新建立打开的远程连接",
        "ja": "開いていたリモート接続を自動的に再確立します",
        "ru": "Автоматически восстанавливать открытые удаленные подключения",
    },
    "Restore Sidebars & AI Panel": {
        "pt": "Restaurar Painéis Laterais e Assistente de IA",
        "en": "Restore Sidebars & AI Panel",
        "es": "Restaurar Paneles Laterales y Asistente de IA",
        "fr": "Restaurer les panneaux latéraux et le panneau IA",
        "de": "Seitenleisten & KI-Panel wiederherstellen",
        "it": "Ripristina barre laterali e pannello IA",
        "zh": "恢复侧边栏和 AI 面板",
        "ja": "サイドバーと AI パネルを復元",
        "ru": "Восстанавливать боковые панели и панель ИИ",
    },
    "Remember sidebar and AI assistant visibility": {
        "pt": "Lembrar visibilidade da barra lateral e do chat de IA",
        "en": "Remember sidebar and AI assistant visibility",
        "es": "Recordar visibilidad de la barra lateral y del chat de IA",
        "fr": "Se souvenir de la visibilité de la barre latérale et du chat IA",
        "de": "Sichtbarkeit von Seitenleiste und KI-Assistent merken",
        "it": "Ricorda la visibilità della barra laterale e dell'assistente IA",
        "zh": "记住侧边栏和 AI 助手的可见状态",
        "ja": "サイドバーと AI アシスタントの表示状態を記憶",
        "ru": "Запоминать видимость боковой панели и ИИ-помощника",
    },
    "Sessão anterior disponível para restauração.": {
        "pt": "Sessão anterior disponível para restauração.",
        "en": "Previous session available for restoration.",
        "es": "Sesión anterior disponible para restaurar.",
        "fr": "Session précédente disponible pour restauration.",
        "de": "Vorherige Sitzung zur Wiederherstellung verfügbar.",
        "it": "Sessione precedente disponibile per il ripristino.",
        "zh": "上一会话可用于恢复。",
        "ja": "前回のセッションを復元できます。",
        "ru": "Предыдущая сессия доступна для восстановления.",
    },
    "Restaurar": {
        "pt": "Restaurar",
        "en": "Restore",
        "es": "Restaurar",
        "fr": "Restaurer",
        "de": "Wiederherstellen",
        "it": "Ripristina",
        "zh": "恢复",
        "ja": "復元",
        "ru": "Восстановить",
    },
}


def get_existing_msgids(po_content: str) -> set[str]:
    """Parse existing msgids from PO file content."""
    msgids = set()
    lines = po_content.splitlines()
    for i, line in enumerate(lines):
        if line.startswith('msgid "') and not line.startswith('msgid ""'):
            msgid = line[7:-1]
            msgids.add(msgid)
    return msgids


def sync_language_po(po_file: Path) -> int:
    """Synchronize missing entries in a .po file."""
    lang_code = po_file.stem
    content = po_file.read_text(encoding="utf-8")
    existing_msgids = get_existing_msgids(content)

    entries_to_add: List[str] = []

    for msgid, lang_dict in NEW_TRANSLATIONS.items():
        if msgid not in existing_msgids:
            # Pick best translation: specific language -> en -> msgid
            msgstr = lang_dict.get(lang_code, lang_dict.get("en", msgid))
            # Format PO block
            entry = f'\n# \nmsgid "{msgid}"\nmsgstr "{msgstr}"\n'
            entries_to_add.append(entry)

    if entries_to_add:
        updated_content = content.rstrip() + "\n" + "".join(entries_to_add)
        po_file.write_text(updated_content, encoding="utf-8")
        return len(entries_to_add)
    return 0


def compile_mo_files() -> int:
    """Compile all .po files to .mo files in internal locale directory."""
    count = 0
    INTERNAL_LOCALE_DIR.mkdir(parents=True, exist_ok=True)
    for po_file in LOCALE_DIR.glob("*.po"):
        lang = po_file.stem
        target_dir = INTERNAL_LOCALE_DIR / lang / "LC_MESSAGES"
        target_dir.mkdir(parents=True, exist_ok=True)
        mo_file = target_dir / "zashterminal.mo"

        try:
            cmd = ["msgfmt", str(po_file), "-o", str(mo_file)]
            subprocess.run(cmd, check=True, capture_output=True)
            count += 1
            # Special case for pt -> pt_BR
            if lang == "pt":
                pt_br_dir = INTERNAL_LOCALE_DIR / "pt_BR" / "LC_MESSAGES"
                pt_br_dir.mkdir(parents=True, exist_ok=True)
                subprocess.run(
                    ["msgfmt", str(po_file), "-o", str(pt_br_dir / "zashterminal.mo")],
                    check=True,
                    capture_output=True,
                )
        except Exception:
            pass
    return count


def main() -> None:
    print("🌐 Sincronizando traduções em todos os 28 idiomas...")
    total_added = 0
    files_updated = 0

    for po_file in sorted(LOCALE_DIR.glob("*.po")):
        added = sync_language_po(po_file)
        if added > 0:
            files_updated += 1
            total_added += added
            print(f"  ✓ {po_file.name}: +{added} chaves traduzidas")
        else:
            print(f"  ✓ {po_file.name}: já sincronizado")

    print(f"\n📊 Total: {files_updated} arquivos .po atualizados com {total_added} novas traduções.")

    print("\n🔨 Compilando catálogos binários .mo...")
    compiled_count = compile_mo_files()
    print(f"✅ {compiled_count} catálogos .mo compilados com sucesso.")


if __name__ == "__main__":
    main()
