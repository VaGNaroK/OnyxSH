# Diretrizes de Desenvolvimento e Testes - OnyxSH

## 🧪 Regra Obrigatória de Testes Unitários:
Sempre que qualquer modificação ou nova funcionalidade for implementada no código:
1. **Elaborar Testes Unitários:** Criar ou atualizar os arquivos de teste em `tests/` cobrindo as classes, métodos e lógicas alteradas.
2. **Executar a Suíte de Testes:** Executar os testes via terminal para validar que funcionou e que nenhum teste regressivo falhou:
   ```bash
   PYTHONPATH=src python3 -m unittest discover -s tests
   ```
3. **Validar antes de Commit:** Apenas prosseguir após confirmação de que todos os testes passaram com 100% de sucesso.
