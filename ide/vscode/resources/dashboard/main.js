(function() {
    const vscode = acquireVsCodeApi();

    window.runCommand = function(command) {
        vscode.postMessage({
            command: 'runCommand',
            value: command
        });
    };

    window.openUrl = function(url) {
        vscode.postMessage({
            command: 'openUrl',
            value: url
        });
    };

    // Listen for messages from the extension
    window.addEventListener('message', event => {
        const message = event.data;
        switch (message.type) {
            case 'updateProject':
                updateProjectInfo(message.data);
                break;
        }
    });

    function updateProjectInfo(data) {
        const infoDiv = document.getElementById('project-info');
        if (!data) {
            infoDiv.innerHTML = '<p>No Pleno project detected in the current workspace.</p><button onclick="runCommand(\'pleno.openWalkthrough\')">Learn how to create one</button>';
            return;
        }

        infoDiv.innerHTML = `
            <div class="project-details">
                <p><strong>Name:</strong> <code>${data.name}</code></p>
                <p><strong>Version:</strong> <code>${data.version}</code></p>
                <p><strong>Main:</strong> <code>${data.main}</code></p>
            </div>
        `;
    }
}());
