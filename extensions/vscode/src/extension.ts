// Copyright 2026 Hub Horizon LLC
// SPDX-License-Identifier: Apache-2.0

import * as vscode from "vscode";

export function activate(context: vscode.ExtensionContext) {
  context.subscriptions.push(
    vscode.commands.registerCommand("squick.scan", async () => {
      const root = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
      if (!root) {
        vscode.window.showWarningMessage("Squick: open a workspace first.");
        return;
      }
      vscode.window.showInformationMessage(
        `Squick scan stub: would scan ${root}`,
      );
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("squick.toggleWatch", () => {
      vscode.window.showInformationMessage("Squick: watch toggle stub.");
    }),
  );

  const handler: vscode.ChatRequestHandler = async (
    request,
    _ctx,
    stream,
    _token,
  ) => {
    stream.markdown(
      `**Squick** stub: received "${request.prompt}". ` +
        `Project context will be injected here once the scanner is wired in.`,
    );
  };

  const participant = vscode.chat.createChatParticipant(
    "squick.context",
    handler,
  );
  participant.iconPath = new vscode.ThemeIcon("symbol-namespace");
  context.subscriptions.push(participant);
}

export function deactivate() {}
