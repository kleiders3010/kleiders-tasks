const fs = require('fs'); const path = require('path');
const short = (e) => String((e && e.message) || e).split(/\r?\n/)[0].slice(0, 140);
const dir = process.argv[2]; const vers = process.argv.slice(3);
function canon(v){ if (Array.isArray(v)) return v.map(canon); if (v instanceof Uint8Array) return Array.from(v); if (v && typeof v==='object') { const o={}; for (const k of Object.keys(v).sort()) o[k]=canon(v[k]); return o;} return v; }
for (const ver of vers) {
  const L = require(path.join(__dirname, 'v'+ver, 'package', 'nodejs', 'index.js'));
  console.log('\n######## loro-crdt', ver);
  for (const name of ['plain','mergeable']) {
    const expect = JSON.stringify(canon(JSON.parse(fs.readFileSync(path.join(dir, name+'.json'),'utf8'))));
    for (const kind of ['snapshot','updates','shallow','stateonly']) {
      const blob = fs.readFileSync(path.join(dir, `${name}.${kind}`));
      const d = new L.LoroDoc(); d.setPeerId(500);
      try { d.import(blob); } catch(e) { console.log(`${name}.${kind}: IMPORT THREW ${short(e)}`); continue; }
      let got; try { got = JSON.stringify(canon(d.toJSON())); } catch(e) { console.log(`${name}.${kind}: toJSON THREW ${short(e)}`); continue; }
      const same = got === expect;
      console.log(`${name}.${kind}: import ok, equal=${same}${same?'':'\n    got='+got}`);
      try {
        d.getMap('meta').set('old_edit_'+ver.replace(/\./g,'_'), true); d.commit();
        const tag = `${ver}.${name}.${kind}`;
        fs.writeFileSync(path.join(dir, tag+'.snap.fromjs'), d.export({mode:'snapshot'}));
        fs.writeFileSync(path.join(dir, tag+'.snap.expect'), JSON.stringify(d.toJSON()));
        fs.writeFileSync(path.join(dir, tag+'.upd.fromjs'), d.export({mode:'update'}));
        fs.writeFileSync(path.join(dir, tag+'.upd.expect'), JSON.stringify(d.toJSON()));
        try { fs.writeFileSync(path.join(dir, tag+'.shal.fromjs'), d.export({mode:'shallow-snapshot', frontiers: d.oplogFrontiers()})); fs.writeFileSync(path.join(dir, tag+'.shal.expect'), JSON.stringify(d.toJSON())); console.log('   old shallow export ok'); } catch(e) { console.log(`   old shallow export THREW ${short(e)}`); }
      } catch(e) { console.log(`   re-export THREW ${short(e)}`); }
    }
  }
  const o = new L.LoroDoc(); o.setPeerId(600);
  const t = o.getTree('tasks'); const n1 = t.createNode(); n1.data.set('title','old'); const n2 = t.createNode(); n2.data.set('title','old2'); n2.move(n1);
  o.getText('note').insert(0,'from '+ver); o.getMovableList('order').push('p'); o.getMovableList('order').push('q'); o.getMovableList('order').move(0,1);
  if (o.getCounter) o.getCounter('cnt').increment(2);
  o.commit();
  fs.writeFileSync(path.join(dir, `${ver}.native.snap.fromjs`), o.export({mode:'snapshot'}));
  fs.writeFileSync(path.join(dir, `${ver}.native.snap.expect`), JSON.stringify(o.toJSON()));
  fs.writeFileSync(path.join(dir, `${ver}.native.upd.fromjs`), o.export({mode:'update'}));
  fs.writeFileSync(path.join(dir, `${ver}.native.upd.expect`), JSON.stringify(o.toJSON()));
  try { fs.writeFileSync(path.join(dir, `${ver}.native.shal.fromjs`), o.export({mode:'shallow-snapshot', frontiers:o.oplogFrontiers()})); fs.writeFileSync(path.join(dir, `${ver}.native.shal.expect`), JSON.stringify(o.toJSON())); } catch(e) { console.log('native shallow THREW', short(e)); }
}
