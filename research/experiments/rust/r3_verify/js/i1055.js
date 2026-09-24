const path = require('path');
for (const ver of process.argv.slice(2)) {
  const { LoroDoc, UndoManager } = require(path.join(__dirname, 'v'+ver, 'package', 'nodejs', 'index.js'));
  const a = new LoroDoc(); a.setPeerId(1);
  const node = a.getTree("tree").createNode(); const treeId = node.id; a.commit();
  const um = new UndoManager(a, { mergeInterval: 0 });
  const body = node.data.ensureMergeableText("body"); body.insert(0, "hello"); a.commit();
  const snapshot = a.export({ mode: "snapshot" }); const forkVV = a.oplogVersion();
  const b = new LoroDoc(); b.setPeerId(2); b.import(snapshot);
  const other = b.getTree("tree").createNode(); other.data.ensureMergeableText("body").insert(0, "unrelated"); b.commit();
  a.import(b.export({ mode: "update", from: forkVV }));
  const fresh = a.getTree("tree").getNodeByID(treeId); const fb = fresh.data.ensureMergeableText("body"); fb.insert(fb.length, " world"); a.commit();
  um.undo();
  console.log(`loro-crdt ${ver}: #1055 original node deleted after undo = ${a.getTree("tree").isNodeDeleted(treeId)}`);
}
