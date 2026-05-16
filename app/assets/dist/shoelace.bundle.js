var re=globalThis,ie=re.trustedTypes,ao=ie?ie.createPolicy("lit-html",{createHTML:t=>t}):void 0,ze="$lit$",at=`lit$${Math.random().toFixed(9).slice(2)}$`,Te="?"+at,Gr=`<${Te}>`,$t=document,jt=()=>$t.createComment(""),qt=t=>t===null||typeof t!="object"&&typeof t!="function",Le=Array.isArray,Eo=t=>Le(t)||typeof t?.[Symbol.iterator]=="function",Ee=`[ 	
\f\r]`,Ut=/<(?:(!--|\/[^a-zA-Z])|(\/?[a-zA-Z][^>\s]*)|(\/?$))/g,co=/-->/g,uo=/>/g,xt=RegExp(`>|${Ee}(?:([^\\s"'>=/]+)(${Ee}*=${Ee}*(?:[^ 	
\f\r"'\`<>=]|("|')|))|$)`,"g"),ho=/'/g,po=/"/g,Oo=/^(?:script|style|textarea|title)$/i,Pe=t=>(e,...o)=>({_$litType$:t,strings:e,values:o}),k=Pe(1),zo=Pe(2),To=Pe(3),rt=Symbol.for("lit-noChange"),z=Symbol.for("lit-nothing"),fo=new WeakMap,Ct=$t.createTreeWalker($t,129);function Lo(t,e){if(!Le(t)||!t.hasOwnProperty("raw"))throw Error("invalid template strings array");return ao!==void 0?ao.createHTML(e):e}var Po=(t,e)=>{let o=t.length-1,r=[],i,s=e===2?"<svg>":e===3?"<math>":"",n=Ut;for(let l=0;l<o;l++){let c=t[l],d,p,h=-1,m=0;for(;m<c.length&&(n.lastIndex=m,p=n.exec(c),p!==null);)m=n.lastIndex,n===Ut?p[1]==="!--"?n=co:p[1]!==void 0?n=uo:p[2]!==void 0?(Oo.test(p[2])&&(i=RegExp("</"+p[2],"g")),n=xt):p[3]!==void 0&&(n=xt):n===xt?p[0]===">"?(n=i??Ut,h=-1):p[1]===void 0?h=-2:(h=n.lastIndex-p[2].length,d=p[1],n=p[3]===void 0?xt:p[3]==='"'?po:ho):n===po||n===ho?n=xt:n===co||n===uo?n=Ut:(n=xt,i=void 0);let f=n===xt&&t[l+1].startsWith("/>")?" ":"";s+=n===Ut?c+Gr:h>=0?(r.push(d),c.slice(0,h)+ze+c.slice(h)+at+f):c+at+(h===-2?l:f)}return[Lo(t,s+(t[o]||"<?>")+(e===2?"</svg>":e===3?"</math>":"")),r]},Oe=class Do{constructor({strings:e,_$litType$:o},r){let i;this.parts=[];let s=0,n=0,l=e.length-1,c=this.parts,[d,p]=Po(e,o);if(this.el=Do.createElement(d,r),Ct.currentNode=this.el.content,o===2||o===3){let h=this.el.content.firstChild;h.replaceWith(...h.childNodes)}for(;(i=Ct.nextNode())!==null&&c.length<l;){if(i.nodeType===1){if(i.hasAttributes())for(let h of i.getAttributeNames())if(h.endsWith(ze)){let m=p[n++],f=i.getAttribute(h).split(at),g=/([.?@])?(.*)/.exec(m);c.push({type:1,index:s,name:g[2],strings:f,ctor:g[1]==="."?Mo:g[1]==="?"?Vo:g[1]==="@"?No:Yt}),i.removeAttribute(h)}else h.startsWith(at)&&(c.push({type:6,index:s}),i.removeAttribute(h));if(Oo.test(i.tagName)){let h=i.textContent.split(at),m=h.length-1;if(m>0){i.textContent=ie?ie.emptyScript:"";for(let f=0;f<m;f++)i.append(h[f],jt()),Ct.nextNode(),c.push({type:2,index:++s});i.append(h[m],jt())}}}else if(i.nodeType===8)if(i.data===Te)c.push({type:2,index:s});else{let h=-1;for(;(h=i.data.indexOf(at,h+1))!==-1;)c.push({type:7,index:s}),h+=at.length-1}s++}}static createElement(e,o){let r=$t.createElement("template");return r.innerHTML=e,r}};function kt(t,e,o=t,r){var i,s,n;if(e===rt)return e;let l=r!==void 0?(i=o._$Co)==null?void 0:i[r]:o._$Cl,c=qt(e)?void 0:e._$litDirective$;return l?.constructor!==c&&((s=l?._$AO)==null||s.call(l,!1),c===void 0?l=void 0:(l=new c(t),l._$AT(t,o,r)),r!==void 0?((n=o._$Co)!=null?n:o._$Co=[])[r]=l:o._$Cl=l),l!==void 0&&(e=kt(t,l._$AS(t,e.values),l,r)),e}var Ro=class{constructor(t,e){this._$AV=[],this._$AN=void 0,this._$AD=t,this._$AM=e}get parentNode(){return this._$AM.parentNode}get _$AU(){return this._$AM._$AU}u(t){var e;let{el:{content:o},parts:r}=this._$AD,i=((e=t?.creationScope)!=null?e:$t).importNode(o,!0);Ct.currentNode=i;let s=Ct.nextNode(),n=0,l=0,c=r[0];for(;c!==void 0;){if(n===c.index){let d;c.type===2?d=new se(s,s.nextSibling,this,t):c.type===1?d=new c.ctor(s,c.name,c.strings,this,t):c.type===6&&(d=new Io(s,this,t)),this._$AV.push(d),c=r[++l]}n!==c?.index&&(s=Ct.nextNode(),n++)}return Ct.currentNode=$t,i}p(t){let e=0;for(let o of this._$AV)o!==void 0&&(o.strings!==void 0?(o._$AI(t,o,e),e+=o.strings.length-2):o._$AI(t[e])),e++}},se=class Bo{get _$AU(){var e,o;return(o=(e=this._$AM)==null?void 0:e._$AU)!=null?o:this._$Cv}constructor(e,o,r,i){var s;this.type=2,this._$AH=z,this._$AN=void 0,this._$AA=e,this._$AB=o,this._$AM=r,this.options=i,this._$Cv=(s=i?.isConnected)!=null?s:!0}get parentNode(){let e=this._$AA.parentNode,o=this._$AM;return o!==void 0&&e?.nodeType===11&&(e=o.parentNode),e}get startNode(){return this._$AA}get endNode(){return this._$AB}_$AI(e,o=this){e=kt(this,e,o),qt(e)?e===z||e==null||e===""?(this._$AH!==z&&this._$AR(),this._$AH=z):e!==this._$AH&&e!==rt&&this._(e):e._$litType$!==void 0?this.$(e):e.nodeType!==void 0?this.T(e):Eo(e)?this.k(e):this._(e)}O(e){return this._$AA.parentNode.insertBefore(e,this._$AB)}T(e){this._$AH!==e&&(this._$AR(),this._$AH=this.O(e))}_(e){this._$AH!==z&&qt(this._$AH)?this._$AA.nextSibling.data=e:this.T($t.createTextNode(e)),this._$AH=e}$(e){var o;let{values:r,_$litType$:i}=e,s=typeof i=="number"?this._$AC(e):(i.el===void 0&&(i.el=Oe.createElement(Lo(i.h,i.h[0]),this.options)),i);if(((o=this._$AH)==null?void 0:o._$AD)===s)this._$AH.p(r);else{let n=new Ro(s,this),l=n.u(this.options);n.p(r),this.T(l),this._$AH=n}}_$AC(e){let o=fo.get(e.strings);return o===void 0&&fo.set(e.strings,o=new Oe(e)),o}k(e){Le(this._$AH)||(this._$AH=[],this._$AR());let o=this._$AH,r,i=0;for(let s of e)i===o.length?o.push(r=new Bo(this.O(jt()),this.O(jt()),this,this.options)):r=o[i],r._$AI(s),i++;i<o.length&&(this._$AR(r&&r._$AB.nextSibling,i),o.length=i)}_$AR(e=this._$AA.nextSibling,o){var r;for((r=this._$AP)==null?void 0:r.call(this,!1,!0,o);e&&e!==this._$AB;){let i=e.nextSibling;e.remove(),e=i}}setConnected(e){var o;this._$AM===void 0&&(this._$Cv=e,(o=this._$AP)==null||o.call(this,e))}},Yt=class{get tagName(){return this.element.tagName}get _$AU(){return this._$AM._$AU}constructor(t,e,o,r,i){this.type=1,this._$AH=z,this._$AN=void 0,this.element=t,this.name=e,this._$AM=r,this.options=i,o.length>2||o[0]!==""||o[1]!==""?(this._$AH=Array(o.length-1).fill(new String),this.strings=o):this._$AH=z}_$AI(t,e=this,o,r){let i=this.strings,s=!1;if(i===void 0)t=kt(this,t,e,0),s=!qt(t)||t!==this._$AH&&t!==rt,s&&(this._$AH=t);else{let n=t,l,c;for(t=i[0],l=0;l<i.length-1;l++)c=kt(this,n[o+l],e,l),c===rt&&(c=this._$AH[l]),s||(s=!qt(c)||c!==this._$AH[l]),c===z?t=z:t!==z&&(t+=(c??"")+i[l+1]),this._$AH[l]=c}s&&!r&&this.j(t)}j(t){t===z?this.element.removeAttribute(this.name):this.element.setAttribute(this.name,t??"")}},Mo=class extends Yt{constructor(){super(...arguments),this.type=3}j(t){this.element[this.name]=t===z?void 0:t}},Vo=class extends Yt{constructor(){super(...arguments),this.type=4}j(t){this.element.toggleAttribute(this.name,!!t&&t!==z)}},No=class extends Yt{constructor(t,e,o,r,i){super(t,e,o,r,i),this.type=5}_$AI(t,e=this){var o;if((t=(o=kt(this,t,e,0))!=null?o:z)===rt)return;let r=this._$AH,i=t===z&&r!==z||t.capture!==r.capture||t.once!==r.once||t.passive!==r.passive,s=t!==z&&(r===z||i);i&&this.element.removeEventListener(this.name,this,r),s&&this.element.addEventListener(this.name,this,t),this._$AH=t}handleEvent(t){var e,o;typeof this._$AH=="function"?this._$AH.call((o=(e=this.options)==null?void 0:e.host)!=null?o:this.element,t):this._$AH.handleEvent(t)}},Io=class{constructor(t,e,o){this.element=t,this.type=6,this._$AN=void 0,this._$AM=e,this.options=o}get _$AU(){return this._$AM._$AU}_$AI(t){kt(this,t)}},Fo={M:ze,P:at,A:Te,C:1,L:Po,R:Ro,D:Eo,V:kt,I:se,H:Yt,N:Vo,U:No,B:Mo,F:Io},mo=re.litHtmlPolyfillSupport,go;mo?.(Oe,se),((go=re.litHtmlVersions)!=null?go:re.litHtmlVersions=[]).push("3.2.1");var Zr=(t,e,o)=>{var r,i;let s=(r=o?.renderBefore)!=null?r:e,n=s._$litPart$;if(n===void 0){let l=(i=o?.renderBefore)!=null?i:null;s._$litPart$=n=new se(e.insertBefore(jt(),l),l,void 0,o??{})}return n._$AI(t),n},oe=globalThis,De=oe.ShadowRoot&&(oe.ShadyCSS===void 0||oe.ShadyCSS.nativeShadow)&&"adoptedStyleSheets"in Document.prototype&&"replace"in CSSStyleSheet.prototype,Re=Symbol(),bo=new WeakMap,Ho=class{constructor(t,e,o){if(this._$cssResult$=!0,o!==Re)throw Error("CSSResult is not constructable. Use `unsafeCSS` or `css` instead.");this.cssText=t,this.t=e}get styleSheet(){let t=this.o,e=this.t;if(De&&t===void 0){let o=e!==void 0&&e.length===1;o&&(t=bo.get(e)),t===void 0&&((this.o=t=new CSSStyleSheet).replaceSync(this.cssText),o&&bo.set(e,t))}return t}toString(){return this.cssText}},Jr=t=>new Ho(typeof t=="string"?t:t+"",void 0,Re),A=(t,...e)=>{let o=t.length===1?t[0]:e.reduce((r,i,s)=>r+(n=>{if(n._$cssResult$===!0)return n.cssText;if(typeof n=="number")return n;throw Error("Value passed to 'css' function must be a 'css' function result: "+n+". Use 'unsafeCSS' to pass non-literal values, but take care to ensure page security.")})(i)+t[s+1],t[0]);return new Ho(o,t,Re)},Qr=(t,e)=>{if(De)t.adoptedStyleSheets=e.map(o=>o instanceof CSSStyleSheet?o:o.styleSheet);else for(let o of e){let r=document.createElement("style"),i=oe.litNonce;i!==void 0&&r.setAttribute("nonce",i),r.textContent=o.cssText,t.appendChild(r)}},vo=De?t=>t:t=>t instanceof CSSStyleSheet?(e=>{let o="";for(let r of e.cssRules)o+=r.cssText;return Jr(o)})(t):t,{is:ti,defineProperty:ei,getOwnPropertyDescriptor:oi,getOwnPropertyNames:ri,getOwnPropertySymbols:ii,getPrototypeOf:si}=Object,Pt=globalThis,yo=Pt.trustedTypes,ni=yo?yo.emptyScript:"",_o=Pt.reactiveElementPolyfillSupport,Wt=(t,e)=>t,Kt={toAttribute(t,e){switch(e){case Boolean:t=t?ni:null;break;case Object:case Array:t=t==null?t:JSON.stringify(t)}return t},fromAttribute(t,e){let o=t;switch(e){case Boolean:o=t!==null;break;case Number:o=t===null?null:Number(t);break;case Object:case Array:try{o=JSON.parse(t)}catch{o=null}}return o}},ne=(t,e)=>!ti(t,e),wo={attribute:!0,type:String,converter:Kt,reflect:!1,hasChanged:ne},xo,Co;(xo=Symbol.metadata)!=null||(Symbol.metadata=Symbol("metadata")),(Co=Pt.litPropertyMetadata)!=null||(Pt.litPropertyMetadata=new WeakMap);var Tt=class extends HTMLElement{static addInitializer(t){var e;this._$Ei(),((e=this.l)!=null?e:this.l=[]).push(t)}static get observedAttributes(){return this.finalize(),this._$Eh&&[...this._$Eh.keys()]}static createProperty(t,e=wo){if(e.state&&(e.attribute=!1),this._$Ei(),this.elementProperties.set(t,e),!e.noAccessor){let o=Symbol(),r=this.getPropertyDescriptor(t,o,e);r!==void 0&&ei(this.prototype,t,r)}}static getPropertyDescriptor(t,e,o){var r;let{get:i,set:s}=(r=oi(this.prototype,t))!=null?r:{get(){return this[e]},set(n){this[e]=n}};return{get(){return i?.call(this)},set(n){let l=i?.call(this);s.call(this,n),this.requestUpdate(t,l,o)},configurable:!0,enumerable:!0}}static getPropertyOptions(t){var e;return(e=this.elementProperties.get(t))!=null?e:wo}static _$Ei(){if(this.hasOwnProperty(Wt("elementProperties")))return;let t=si(this);t.finalize(),t.l!==void 0&&(this.l=[...t.l]),this.elementProperties=new Map(t.elementProperties)}static finalize(){if(this.hasOwnProperty(Wt("finalized")))return;if(this.finalized=!0,this._$Ei(),this.hasOwnProperty(Wt("properties"))){let e=this.properties,o=[...ri(e),...ii(e)];for(let r of o)this.createProperty(r,e[r])}let t=this[Symbol.metadata];if(t!==null){let e=litPropertyMetadata.get(t);if(e!==void 0)for(let[o,r]of e)this.elementProperties.set(o,r)}this._$Eh=new Map;for(let[e,o]of this.elementProperties){let r=this._$Eu(e,o);r!==void 0&&this._$Eh.set(r,e)}this.elementStyles=this.finalizeStyles(this.styles)}static finalizeStyles(t){let e=[];if(Array.isArray(t)){let o=new Set(t.flat(1/0).reverse());for(let r of o)e.unshift(vo(r))}else t!==void 0&&e.push(vo(t));return e}static _$Eu(t,e){let o=e.attribute;return o===!1?void 0:typeof o=="string"?o:typeof t=="string"?t.toLowerCase():void 0}constructor(){super(),this._$Ep=void 0,this.isUpdatePending=!1,this.hasUpdated=!1,this._$Em=null,this._$Ev()}_$Ev(){var t;this._$ES=new Promise(e=>this.enableUpdating=e),this._$AL=new Map,this._$E_(),this.requestUpdate(),(t=this.constructor.l)==null||t.forEach(e=>e(this))}addController(t){var e,o;((e=this._$EO)!=null?e:this._$EO=new Set).add(t),this.renderRoot!==void 0&&this.isConnected&&((o=t.hostConnected)==null||o.call(t))}removeController(t){var e;(e=this._$EO)==null||e.delete(t)}_$E_(){let t=new Map,e=this.constructor.elementProperties;for(let o of e.keys())this.hasOwnProperty(o)&&(t.set(o,this[o]),delete this[o]);t.size>0&&(this._$Ep=t)}createRenderRoot(){var t;let e=(t=this.shadowRoot)!=null?t:this.attachShadow(this.constructor.shadowRootOptions);return Qr(e,this.constructor.elementStyles),e}connectedCallback(){var t,e;(t=this.renderRoot)!=null||(this.renderRoot=this.createRenderRoot()),this.enableUpdating(!0),(e=this._$EO)==null||e.forEach(o=>{var r;return(r=o.hostConnected)==null?void 0:r.call(o)})}enableUpdating(t){}disconnectedCallback(){var t;(t=this._$EO)==null||t.forEach(e=>{var o;return(o=e.hostDisconnected)==null?void 0:o.call(e)})}attributeChangedCallback(t,e,o){this._$AK(t,o)}_$EC(t,e){var o;let r=this.constructor.elementProperties.get(t),i=this.constructor._$Eu(t,r);if(i!==void 0&&r.reflect===!0){let s=(((o=r.converter)==null?void 0:o.toAttribute)!==void 0?r.converter:Kt).toAttribute(e,r.type);this._$Em=t,s==null?this.removeAttribute(i):this.setAttribute(i,s),this._$Em=null}}_$AK(t,e){var o;let r=this.constructor,i=r._$Eh.get(t);if(i!==void 0&&this._$Em!==i){let s=r.getPropertyOptions(i),n=typeof s.converter=="function"?{fromAttribute:s.converter}:((o=s.converter)==null?void 0:o.fromAttribute)!==void 0?s.converter:Kt;this._$Em=i,this[i]=n.fromAttribute(e,s.type),this._$Em=null}}requestUpdate(t,e,o){var r;if(t!==void 0){if(o??(o=this.constructor.getPropertyOptions(t)),!((r=o.hasChanged)!=null?r:ne)(this[t],e))return;this.P(t,e,o)}this.isUpdatePending===!1&&(this._$ES=this._$ET())}P(t,e,o){var r;this._$AL.has(t)||this._$AL.set(t,e),o.reflect===!0&&this._$Em!==t&&((r=this._$Ej)!=null?r:this._$Ej=new Set).add(t)}async _$ET(){this.isUpdatePending=!0;try{await this._$ES}catch(e){Promise.reject(e)}let t=this.scheduleUpdate();return t!=null&&await t,!this.isUpdatePending}scheduleUpdate(){return this.performUpdate()}performUpdate(){var t,e;if(!this.isUpdatePending)return;if(!this.hasUpdated){if((t=this.renderRoot)!=null||(this.renderRoot=this.createRenderRoot()),this._$Ep){for(let[s,n]of this._$Ep)this[s]=n;this._$Ep=void 0}let i=this.constructor.elementProperties;if(i.size>0)for(let[s,n]of i)n.wrapped!==!0||this._$AL.has(s)||this[s]===void 0||this.P(s,this[s],n)}let o=!1,r=this._$AL;try{o=this.shouldUpdate(r),o?(this.willUpdate(r),(e=this._$EO)==null||e.forEach(i=>{var s;return(s=i.hostUpdate)==null?void 0:s.call(i)}),this.update(r)):this._$EU()}catch(i){throw o=!1,this._$EU(),i}o&&this._$AE(r)}willUpdate(t){}_$AE(t){var e;(e=this._$EO)==null||e.forEach(o=>{var r;return(r=o.hostUpdated)==null?void 0:r.call(o)}),this.hasUpdated||(this.hasUpdated=!0,this.firstUpdated(t)),this.updated(t)}_$EU(){this._$AL=new Map,this.isUpdatePending=!1}get updateComplete(){return this.getUpdateComplete()}getUpdateComplete(){return this._$ES}shouldUpdate(t){return!0}update(t){this._$Ej&&(this._$Ej=this._$Ej.forEach(e=>this._$EC(e,this[e]))),this._$EU()}updated(t){}firstUpdated(t){}},$o;Tt.elementStyles=[],Tt.shadowRootOptions={mode:"open"},Tt[Wt("elementProperties")]=new Map,Tt[Wt("finalized")]=new Map,_o?.({ReactiveElement:Tt}),(($o=Pt.reactiveElementVersions)!=null?$o:Pt.reactiveElementVersions=[]).push("2.0.4");var Lt=class extends Tt{constructor(){super(...arguments),this.renderOptions={host:this},this._$Do=void 0}createRenderRoot(){var t,e;let o=super.createRenderRoot();return(e=(t=this.renderOptions).renderBefore)!=null||(t.renderBefore=o.firstChild),o}update(t){let e=this.render();this.hasUpdated||(this.renderOptions.isConnected=this.isConnected),super.update(t),this._$Do=Zr(e,this.renderRoot,this.renderOptions)}connectedCallback(){var t;super.connectedCallback(),(t=this._$Do)==null||t.setConnected(!0)}disconnectedCallback(){var t;super.disconnectedCallback(),(t=this._$Do)==null||t.setConnected(!1)}render(){return rt}},ko;Lt._$litElement$=!0,Lt.finalized=!0,(ko=globalThis.litElementHydrateSupport)==null||ko.call(globalThis,{LitElement:Lt});var So=globalThis.litElementPolyfillSupport;So?.({LitElement:Lt});var Ao;((Ao=globalThis.litElementVersions)!=null?Ao:globalThis.litElementVersions=[]).push("4.1.1");var Uo=A`
  :host {
    --track-width: 2px;
    --track-color: rgb(128 128 128 / 25%);
    --indicator-color: var(--sl-color-primary-600);
    --speed: 2s;

    display: inline-flex;
    width: 1em;
    height: 1em;
    flex: none;
  }

  .spinner {
    flex: 1 1 auto;
    height: 100%;
    width: 100%;
  }

  .spinner__track,
  .spinner__indicator {
    fill: none;
    stroke-width: var(--track-width);
    r: calc(0.5em - var(--track-width) / 2);
    cx: 0.5em;
    cy: 0.5em;
    transform-origin: 50% 50%;
  }

  .spinner__track {
    stroke: var(--track-color);
    transform-origin: 0% 0%;
  }

  .spinner__indicator {
    stroke: var(--indicator-color);
    stroke-linecap: round;
    stroke-dasharray: 150% 75%;
    animation: spin var(--speed) linear infinite;
  }

  @keyframes spin {
    0% {
      transform: rotate(0deg);
      stroke-dasharray: 0.05em, 3em;
    }

    50% {
      transform: rotate(450deg);
      stroke-dasharray: 1.375em, 1.375em;
    }

    100% {
      transform: rotate(1080deg);
      stroke-dasharray: 0.05em, 3em;
    }
  }
`;var Be=new Set,Dt=new Map,St,Me="ltr",Ve="en",Wo=typeof MutationObserver<"u"&&typeof document<"u"&&typeof document.documentElement<"u";if(Wo){let t=new MutationObserver(jo);Me=document.documentElement.dir||"ltr",Ve=document.documentElement.lang||navigator.language,t.observe(document.documentElement,{attributes:!0,attributeFilter:["dir","lang"]})}function le(...t){t.map(e=>{let o=e.$code.toLowerCase();Dt.has(o)?Dt.set(o,Object.assign(Object.assign({},Dt.get(o)),e)):Dt.set(o,e),St||(St=e)}),jo()}function jo(){Wo&&(Me=document.documentElement.dir||"ltr",Ve=document.documentElement.lang||navigator.language),[...Be.keys()].map(t=>{typeof t.requestUpdate=="function"&&t.requestUpdate()})}var qo=class{constructor(t){this.host=t,this.host.addController(this)}hostConnected(){Be.add(this.host)}hostDisconnected(){Be.delete(this.host)}dir(){return`${this.host.dir||Me}`.toLowerCase()}lang(){return`${this.host.lang||Ve}`.toLowerCase()}getTranslationData(t){var e,o;let r=new Intl.Locale(t.replace(/_/g,"-")),i=r?.language.toLowerCase(),s=(o=(e=r?.region)===null||e===void 0?void 0:e.toLowerCase())!==null&&o!==void 0?o:"",n=Dt.get(`${i}-${s}`),l=Dt.get(i);return{locale:r,language:i,region:s,primary:n,secondary:l}}exists(t,e){var o;let{primary:r,secondary:i}=this.getTranslationData((o=e.lang)!==null&&o!==void 0?o:this.lang());return e=Object.assign({includeFallback:!1},e),!!(r&&r[t]||i&&i[t]||e.includeFallback&&St&&St[t])}term(t,...e){let{primary:o,secondary:r}=this.getTranslationData(this.lang()),i;if(o&&o[t])i=o[t];else if(r&&r[t])i=r[t];else if(St&&St[t])i=St[t];else return console.error(`No translation found for: ${String(t)}`),String(t);return typeof i=="function"?i(...e):i}date(t,e){return t=new Date(t),new Intl.DateTimeFormat(this.lang(),e).format(t)}number(t,e){return t=Number(t),isNaN(t)?"":new Intl.NumberFormat(this.lang(),e).format(t)}relativeTime(t,e,o){return new Intl.RelativeTimeFormat(this.lang(),o).format(t,e)}};var Ko={$code:"en",$name:"English",$dir:"ltr",carousel:"Carousel",clearEntry:"Clear entry",close:"Close",copied:"Copied",copy:"Copy",currentValue:"Current value",error:"Error",goToSlide:(t,e)=>`Go to slide ${t} of ${e}`,hidePassword:"Hide password",loading:"Loading",nextSlide:"Next slide",numOptionsSelected:t=>t===0?"No options selected":t===1?"1 option selected":`${t} options selected`,previousSlide:"Previous slide",progress:"Progress",remove:"Remove",resize:"Resize",scrollToEnd:"Scroll to end",scrollToStart:"Scroll to start",selectAColorFromTheScreen:"Select a color from the screen",showPassword:"Show password",slideNum:t=>`Slide ${t}`,toggleColorFormat:"Toggle color format"};le(Ko);var Yo=Ko;var F=class extends qo{};le(Yo);var D=A`
  :host {
    box-sizing: border-box;
  }

  :host *,
  :host *::before,
  :host *::after {
    box-sizing: inherit;
  }

  [hidden] {
    display: none !important;
  }
`;var Go=Object.defineProperty,li=Object.defineProperties,ai=Object.getOwnPropertyDescriptor,ci=Object.getOwnPropertyDescriptors,ae=Object.getOwnPropertySymbols,Zo=Object.prototype.hasOwnProperty,Jo=Object.prototype.propertyIsEnumerable,Ne=(t,e)=>(e=Symbol[t])?e:Symbol.for("Symbol."+t),Ie=t=>{throw TypeError(t)},Xo=(t,e,o)=>e in t?Go(t,e,{enumerable:!0,configurable:!0,writable:!0,value:o}):t[e]=o,S=(t,e)=>{for(var o in e||(e={}))Zo.call(e,o)&&Xo(t,o,e[o]);if(ae)for(var o of ae(e))Jo.call(e,o)&&Xo(t,o,e[o]);return t},B=(t,e)=>li(t,ci(e)),ce=(t,e)=>{var o={};for(var r in t)Zo.call(t,r)&&e.indexOf(r)<0&&(o[r]=t[r]);if(t!=null&&ae)for(var r of ae(t))e.indexOf(r)<0&&Jo.call(t,r)&&(o[r]=t[r]);return o};var a=(t,e,o,r)=>{for(var i=r>1?void 0:r?ai(e,o):e,s=t.length-1,n;s>=0;s--)(n=t[s])&&(i=(r?n(e,o,i):n(i))||i);return r&&i&&Go(e,o,i),i},Qo=(t,e,o)=>e.has(t)||Ie("Cannot "+o),tr=(t,e,o)=>(Qo(t,e,"read from private field"),o?o.call(t):e.get(t)),er=(t,e,o)=>e.has(t)?Ie("Cannot add the same private member more than once"):e instanceof WeakSet?e.add(t):e.set(t,o),or=(t,e,o,r)=>(Qo(t,e,"write to private field"),r?r.call(t,o):e.set(t,o),o),ui=function(t,e){this[0]=t,this[1]=e},rr=t=>{var e=t[Ne("asyncIterator")],o=!1,r,i={};return e==null?(e=t[Ne("iterator")](),r=s=>i[s]=n=>e[s](n)):(e=e.call(t),r=s=>i[s]=n=>{if(o){if(o=!1,s==="throw")throw n;return n}return o=!0,{done:!1,value:new ui(new Promise(l=>{var c=e[s](n);c instanceof Object||Ie("Object expected"),l(c)}),1)}}),i[Ne("iterator")]=()=>i,r("next"),"throw"in e?r("throw"):i.throw=s=>{throw s},"return"in e&&r("return"),i};var di={attribute:!0,type:String,converter:Kt,reflect:!1,hasChanged:ne},hi=(t=di,e,o)=>{let{kind:r,metadata:i}=o,s=globalThis.litPropertyMetadata.get(i);if(s===void 0&&globalThis.litPropertyMetadata.set(i,s=new Map),s.set(o.name,t),r==="accessor"){let{name:n}=o;return{set(l){let c=e.get.call(this);e.set.call(this,l),this.requestUpdate(n,c,t)},init(l){return l!==void 0&&this.P(n,void 0,t),l}}}if(r==="setter"){let{name:n}=o;return function(l){let c=this[n];e.call(this,l),this.requestUpdate(n,c,t)}}throw Error("Unsupported decorator location: "+r)};function u(t){return(e,o)=>typeof o=="object"?hi(t,e,o):((r,i,s)=>{let n=i.hasOwnProperty(s);return i.constructor.createProperty(s,n?B(S({},r),{wrapped:!0}):r),n?Object.getOwnPropertyDescriptor(i,s):void 0})(t,e,o)}function R(t){return u(B(S({},t),{state:!0,attribute:!1}))}var ir=(t,e,o)=>(o.configurable=!0,o.enumerable=!0,Reflect.decorate&&typeof e!="object"&&Object.defineProperty(t,e,o),o);function O(t,e){return(o,r,i)=>{let s=n=>{var l,c;return(c=(l=n.renderRoot)==null?void 0:l.querySelector(t))!=null?c:null};if(e){let{get:n,set:l}=typeof r=="object"?o:i??(()=>{let c=Symbol();return{get(){return this[c]},set(d){this[c]=d}}})();return ir(o,r,{get(){let c=n.call(this);return c===void 0&&(c=s(this),(c!==null||this.hasUpdated)&&l.call(this,c)),c}})}return ir(o,r,{get(){return s(this)}})}}var ue,E=class extends Lt{constructor(){super(),er(this,ue,!1),this.initialReflectedProperties=new Map,Object.entries(this.constructor.dependencies).forEach(([t,e])=>{this.constructor.define(t,e)})}emit(t,e){let o=new CustomEvent(t,S({bubbles:!0,cancelable:!1,composed:!0,detail:{}},e));return this.dispatchEvent(o),o}static define(t,e=this,o={}){let r=customElements.get(t);if(!r){try{customElements.define(t,e,o)}catch{customElements.define(t,class extends e{},o)}return}let i=" (unknown version)",s=i;"version"in e&&e.version&&(i=" v"+e.version),"version"in r&&r.version&&(s=" v"+r.version),!(i&&s&&i===s)&&console.warn(`Attempted to register <${t}>${i}, but <${t}>${s} has already been registered.`)}attributeChangedCallback(t,e,o){tr(this,ue)||(this.constructor.elementProperties.forEach((r,i)=>{r.reflect&&this[i]!=null&&this.initialReflectedProperties.set(i,this[i])}),or(this,ue,!0)),super.attributeChangedCallback(t,e,o)}willUpdate(t){super.willUpdate(t),this.initialReflectedProperties.forEach((e,o)=>{t.has(o)&&this[o]==null&&(this[o]=e)})}};ue=new WeakMap;E.version="2.20.1";E.dependencies={};a([u()],E.prototype,"dir",2);a([u()],E.prototype,"lang",2);var Fe=class extends E{constructor(){super(...arguments),this.localize=new F(this)}render(){return k`
      <svg part="base" class="spinner" role="progressbar" aria-label=${this.localize.term("loading")}>
        <circle class="spinner__track"></circle>
        <circle class="spinner__indicator"></circle>
      </svg>
    `}};Fe.styles=[D,Uo];var Xt=new WeakMap,Gt=new WeakMap,Zt=new WeakMap,He=new WeakSet,de=new WeakMap,he=class{constructor(t,e){this.handleFormData=o=>{let r=this.options.disabled(this.host),i=this.options.name(this.host),s=this.options.value(this.host),n=this.host.tagName.toLowerCase()==="sl-button";this.host.isConnected&&!r&&!n&&typeof i=="string"&&i.length>0&&typeof s<"u"&&(Array.isArray(s)?s.forEach(l=>{o.formData.append(i,l.toString())}):o.formData.append(i,s.toString()))},this.handleFormSubmit=o=>{var r;let i=this.options.disabled(this.host),s=this.options.reportValidity;this.form&&!this.form.noValidate&&((r=Xt.get(this.form))==null||r.forEach(n=>{this.setUserInteracted(n,!0)})),this.form&&!this.form.noValidate&&!i&&!s(this.host)&&(o.preventDefault(),o.stopImmediatePropagation())},this.handleFormReset=()=>{this.options.setValue(this.host,this.options.defaultValue(this.host)),this.setUserInteracted(this.host,!1),de.set(this.host,[])},this.handleInteraction=o=>{let r=de.get(this.host);r.includes(o.type)||r.push(o.type),r.length===this.options.assumeInteractionOn.length&&this.setUserInteracted(this.host,!0)},this.checkFormValidity=()=>{if(this.form&&!this.form.noValidate){let o=this.form.querySelectorAll("*");for(let r of o)if(typeof r.checkValidity=="function"&&!r.checkValidity())return!1}return!0},this.reportFormValidity=()=>{if(this.form&&!this.form.noValidate){let o=this.form.querySelectorAll("*");for(let r of o)if(typeof r.reportValidity=="function"&&!r.reportValidity())return!1}return!0},(this.host=t).addController(this),this.options=S({form:o=>{let r=o.form;if(r){let s=o.getRootNode().querySelector(`#${r}`);if(s)return s}return o.closest("form")},name:o=>o.name,value:o=>o.value,defaultValue:o=>o.defaultValue,disabled:o=>{var r;return(r=o.disabled)!=null?r:!1},reportValidity:o=>typeof o.reportValidity=="function"?o.reportValidity():!0,checkValidity:o=>typeof o.checkValidity=="function"?o.checkValidity():!0,setValue:(o,r)=>o.value=r,assumeInteractionOn:["sl-input"]},e)}hostConnected(){let t=this.options.form(this.host);t&&this.attachForm(t),de.set(this.host,[]),this.options.assumeInteractionOn.forEach(e=>{this.host.addEventListener(e,this.handleInteraction)})}hostDisconnected(){this.detachForm(),de.delete(this.host),this.options.assumeInteractionOn.forEach(t=>{this.host.removeEventListener(t,this.handleInteraction)})}hostUpdated(){let t=this.options.form(this.host);t||this.detachForm(),t&&this.form!==t&&(this.detachForm(),this.attachForm(t)),this.host.hasUpdated&&this.setValidity(this.host.validity.valid)}attachForm(t){t?(this.form=t,Xt.has(this.form)?Xt.get(this.form).add(this.host):Xt.set(this.form,new Set([this.host])),this.form.addEventListener("formdata",this.handleFormData),this.form.addEventListener("submit",this.handleFormSubmit),this.form.addEventListener("reset",this.handleFormReset),Gt.has(this.form)||(Gt.set(this.form,this.form.reportValidity),this.form.reportValidity=()=>this.reportFormValidity()),Zt.has(this.form)||(Zt.set(this.form,this.form.checkValidity),this.form.checkValidity=()=>this.checkFormValidity())):this.form=void 0}detachForm(){if(!this.form)return;let t=Xt.get(this.form);t&&(t.delete(this.host),t.size<=0&&(this.form.removeEventListener("formdata",this.handleFormData),this.form.removeEventListener("submit",this.handleFormSubmit),this.form.removeEventListener("reset",this.handleFormReset),Gt.has(this.form)&&(this.form.reportValidity=Gt.get(this.form),Gt.delete(this.form)),Zt.has(this.form)&&(this.form.checkValidity=Zt.get(this.form),Zt.delete(this.form)),this.form=void 0))}setUserInteracted(t,e){e?He.add(t):He.delete(t),t.requestUpdate()}doAction(t,e){if(this.form){let o=document.createElement("button");o.type=t,o.style.position="absolute",o.style.width="0",o.style.height="0",o.style.clipPath="inset(50%)",o.style.overflow="hidden",o.style.whiteSpace="nowrap",e&&(o.name=e.name,o.value=e.value,["formaction","formenctype","formmethod","formnovalidate","formtarget"].forEach(r=>{e.hasAttribute(r)&&o.setAttribute(r,e.getAttribute(r))})),this.form.append(o),o.click(),o.remove()}}getForm(){var t;return(t=this.form)!=null?t:null}reset(t){this.doAction("reset",t)}submit(t){this.doAction("submit",t)}setValidity(t){let e=this.host,o=!!He.has(e),r=!!e.required;e.toggleAttribute("data-required",r),e.toggleAttribute("data-optional",!r),e.toggleAttribute("data-invalid",!t),e.toggleAttribute("data-valid",t),e.toggleAttribute("data-user-invalid",!t&&o),e.toggleAttribute("data-user-valid",t&&o)}updateValidity(){let t=this.host;this.setValidity(t.validity.valid)}emitInvalidEvent(t){let e=new CustomEvent("sl-invalid",{bubbles:!1,composed:!1,cancelable:!0,detail:{}});t||e.preventDefault(),this.host.dispatchEvent(e)||t?.preventDefault()}},pe=Object.freeze({badInput:!1,customError:!1,patternMismatch:!1,rangeOverflow:!1,rangeUnderflow:!1,stepMismatch:!1,tooLong:!1,tooShort:!1,typeMismatch:!1,valid:!0,valueMissing:!1}),Ks=Object.freeze(B(S({},pe),{valid:!1,valueMissing:!0})),Ys=Object.freeze(B(S({},pe),{valid:!1,customError:!0}));var sr=A`
  :host {
    display: inline-block;
    position: relative;
    width: auto;
    cursor: pointer;
  }

  .button {
    display: inline-flex;
    align-items: stretch;
    justify-content: center;
    width: 100%;
    border-style: solid;
    border-width: var(--sl-input-border-width);
    font-family: var(--sl-input-font-family);
    font-weight: var(--sl-font-weight-semibold);
    text-decoration: none;
    user-select: none;
    -webkit-user-select: none;
    white-space: nowrap;
    vertical-align: middle;
    padding: 0;
    transition:
      var(--sl-transition-x-fast) background-color,
      var(--sl-transition-x-fast) color,
      var(--sl-transition-x-fast) border,
      var(--sl-transition-x-fast) box-shadow;
    cursor: inherit;
  }

  .button::-moz-focus-inner {
    border: 0;
  }

  .button:focus {
    outline: none;
  }

  .button:focus-visible {
    outline: var(--sl-focus-ring);
    outline-offset: var(--sl-focus-ring-offset);
  }

  .button--disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* When disabled, prevent mouse events from bubbling up from children */
  .button--disabled * {
    pointer-events: none;
  }

  .button__prefix,
  .button__suffix {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    pointer-events: none;
  }

  .button__label {
    display: inline-block;
  }

  .button__label::slotted(sl-icon) {
    vertical-align: -2px;
  }

  /*
   * Standard buttons
   */

  /* Default */
  .button--standard.button--default {
    background-color: var(--sl-color-neutral-0);
    border-color: var(--sl-input-border-color);
    color: var(--sl-color-neutral-700);
  }

  .button--standard.button--default:hover:not(.button--disabled) {
    background-color: var(--sl-color-primary-50);
    border-color: var(--sl-color-primary-300);
    color: var(--sl-color-primary-700);
  }

  .button--standard.button--default:active:not(.button--disabled) {
    background-color: var(--sl-color-primary-100);
    border-color: var(--sl-color-primary-400);
    color: var(--sl-color-primary-700);
  }

  /* Primary */
  .button--standard.button--primary {
    background-color: var(--sl-color-primary-600);
    border-color: var(--sl-color-primary-600);
    color: var(--sl-color-neutral-0);
  }

  .button--standard.button--primary:hover:not(.button--disabled) {
    background-color: var(--sl-color-primary-500);
    border-color: var(--sl-color-primary-500);
    color: var(--sl-color-neutral-0);
  }

  .button--standard.button--primary:active:not(.button--disabled) {
    background-color: var(--sl-color-primary-600);
    border-color: var(--sl-color-primary-600);
    color: var(--sl-color-neutral-0);
  }

  /* Success */
  .button--standard.button--success {
    background-color: var(--sl-color-success-600);
    border-color: var(--sl-color-success-600);
    color: var(--sl-color-neutral-0);
  }

  .button--standard.button--success:hover:not(.button--disabled) {
    background-color: var(--sl-color-success-500);
    border-color: var(--sl-color-success-500);
    color: var(--sl-color-neutral-0);
  }

  .button--standard.button--success:active:not(.button--disabled) {
    background-color: var(--sl-color-success-600);
    border-color: var(--sl-color-success-600);
    color: var(--sl-color-neutral-0);
  }

  /* Neutral */
  .button--standard.button--neutral {
    background-color: var(--sl-color-neutral-600);
    border-color: var(--sl-color-neutral-600);
    color: var(--sl-color-neutral-0);
  }

  .button--standard.button--neutral:hover:not(.button--disabled) {
    background-color: var(--sl-color-neutral-500);
    border-color: var(--sl-color-neutral-500);
    color: var(--sl-color-neutral-0);
  }

  .button--standard.button--neutral:active:not(.button--disabled) {
    background-color: var(--sl-color-neutral-600);
    border-color: var(--sl-color-neutral-600);
    color: var(--sl-color-neutral-0);
  }

  /* Warning */
  .button--standard.button--warning {
    background-color: var(--sl-color-warning-600);
    border-color: var(--sl-color-warning-600);
    color: var(--sl-color-neutral-0);
  }
  .button--standard.button--warning:hover:not(.button--disabled) {
    background-color: var(--sl-color-warning-500);
    border-color: var(--sl-color-warning-500);
    color: var(--sl-color-neutral-0);
  }

  .button--standard.button--warning:active:not(.button--disabled) {
    background-color: var(--sl-color-warning-600);
    border-color: var(--sl-color-warning-600);
    color: var(--sl-color-neutral-0);
  }

  /* Danger */
  .button--standard.button--danger {
    background-color: var(--sl-color-danger-600);
    border-color: var(--sl-color-danger-600);
    color: var(--sl-color-neutral-0);
  }

  .button--standard.button--danger:hover:not(.button--disabled) {
    background-color: var(--sl-color-danger-500);
    border-color: var(--sl-color-danger-500);
    color: var(--sl-color-neutral-0);
  }

  .button--standard.button--danger:active:not(.button--disabled) {
    background-color: var(--sl-color-danger-600);
    border-color: var(--sl-color-danger-600);
    color: var(--sl-color-neutral-0);
  }

  /*
   * Outline buttons
   */

  .button--outline {
    background: none;
    border: solid 1px;
  }

  /* Default */
  .button--outline.button--default {
    border-color: var(--sl-input-border-color);
    color: var(--sl-color-neutral-700);
  }

  .button--outline.button--default:hover:not(.button--disabled),
  .button--outline.button--default.button--checked:not(.button--disabled) {
    border-color: var(--sl-color-primary-600);
    background-color: var(--sl-color-primary-600);
    color: var(--sl-color-neutral-0);
  }

  .button--outline.button--default:active:not(.button--disabled) {
    border-color: var(--sl-color-primary-700);
    background-color: var(--sl-color-primary-700);
    color: var(--sl-color-neutral-0);
  }

  /* Primary */
  .button--outline.button--primary {
    border-color: var(--sl-color-primary-600);
    color: var(--sl-color-primary-600);
  }

  .button--outline.button--primary:hover:not(.button--disabled),
  .button--outline.button--primary.button--checked:not(.button--disabled) {
    background-color: var(--sl-color-primary-600);
    color: var(--sl-color-neutral-0);
  }

  .button--outline.button--primary:active:not(.button--disabled) {
    border-color: var(--sl-color-primary-700);
    background-color: var(--sl-color-primary-700);
    color: var(--sl-color-neutral-0);
  }

  /* Success */
  .button--outline.button--success {
    border-color: var(--sl-color-success-600);
    color: var(--sl-color-success-600);
  }

  .button--outline.button--success:hover:not(.button--disabled),
  .button--outline.button--success.button--checked:not(.button--disabled) {
    background-color: var(--sl-color-success-600);
    color: var(--sl-color-neutral-0);
  }

  .button--outline.button--success:active:not(.button--disabled) {
    border-color: var(--sl-color-success-700);
    background-color: var(--sl-color-success-700);
    color: var(--sl-color-neutral-0);
  }

  /* Neutral */
  .button--outline.button--neutral {
    border-color: var(--sl-color-neutral-600);
    color: var(--sl-color-neutral-600);
  }

  .button--outline.button--neutral:hover:not(.button--disabled),
  .button--outline.button--neutral.button--checked:not(.button--disabled) {
    background-color: var(--sl-color-neutral-600);
    color: var(--sl-color-neutral-0);
  }

  .button--outline.button--neutral:active:not(.button--disabled) {
    border-color: var(--sl-color-neutral-700);
    background-color: var(--sl-color-neutral-700);
    color: var(--sl-color-neutral-0);
  }

  /* Warning */
  .button--outline.button--warning {
    border-color: var(--sl-color-warning-600);
    color: var(--sl-color-warning-600);
  }

  .button--outline.button--warning:hover:not(.button--disabled),
  .button--outline.button--warning.button--checked:not(.button--disabled) {
    background-color: var(--sl-color-warning-600);
    color: var(--sl-color-neutral-0);
  }

  .button--outline.button--warning:active:not(.button--disabled) {
    border-color: var(--sl-color-warning-700);
    background-color: var(--sl-color-warning-700);
    color: var(--sl-color-neutral-0);
  }

  /* Danger */
  .button--outline.button--danger {
    border-color: var(--sl-color-danger-600);
    color: var(--sl-color-danger-600);
  }

  .button--outline.button--danger:hover:not(.button--disabled),
  .button--outline.button--danger.button--checked:not(.button--disabled) {
    background-color: var(--sl-color-danger-600);
    color: var(--sl-color-neutral-0);
  }

  .button--outline.button--danger:active:not(.button--disabled) {
    border-color: var(--sl-color-danger-700);
    background-color: var(--sl-color-danger-700);
    color: var(--sl-color-neutral-0);
  }

  @media (forced-colors: active) {
    .button.button--outline.button--checked:not(.button--disabled) {
      outline: solid 2px transparent;
    }
  }

  /*
   * Text buttons
   */

  .button--text {
    background-color: transparent;
    border-color: transparent;
    color: var(--sl-color-primary-600);
  }

  .button--text:hover:not(.button--disabled) {
    background-color: transparent;
    border-color: transparent;
    color: var(--sl-color-primary-500);
  }

  .button--text:focus-visible:not(.button--disabled) {
    background-color: transparent;
    border-color: transparent;
    color: var(--sl-color-primary-500);
  }

  .button--text:active:not(.button--disabled) {
    background-color: transparent;
    border-color: transparent;
    color: var(--sl-color-primary-700);
  }

  /*
   * Size modifiers
   */

  .button--small {
    height: auto;
    min-height: var(--sl-input-height-small);
    font-size: var(--sl-button-font-size-small);
    line-height: calc(var(--sl-input-height-small) - var(--sl-input-border-width) * 2);
    border-radius: var(--sl-input-border-radius-small);
  }

  .button--medium {
    height: auto;
    min-height: var(--sl-input-height-medium);
    font-size: var(--sl-button-font-size-medium);
    line-height: calc(var(--sl-input-height-medium) - var(--sl-input-border-width) * 2);
    border-radius: var(--sl-input-border-radius-medium);
  }

  .button--large {
    height: auto;
    min-height: var(--sl-input-height-large);
    font-size: var(--sl-button-font-size-large);
    line-height: calc(var(--sl-input-height-large) - var(--sl-input-border-width) * 2);
    border-radius: var(--sl-input-border-radius-large);
  }

  /*
   * Pill modifier
   */

  .button--pill.button--small {
    border-radius: var(--sl-input-height-small);
  }

  .button--pill.button--medium {
    border-radius: var(--sl-input-height-medium);
  }

  .button--pill.button--large {
    border-radius: var(--sl-input-height-large);
  }

  /*
   * Circle modifier
   */

  .button--circle {
    padding-left: 0;
    padding-right: 0;
  }

  .button--circle.button--small {
    width: var(--sl-input-height-small);
    border-radius: 50%;
  }

  .button--circle.button--medium {
    width: var(--sl-input-height-medium);
    border-radius: 50%;
  }

  .button--circle.button--large {
    width: var(--sl-input-height-large);
    border-radius: 50%;
  }

  .button--circle .button__prefix,
  .button--circle .button__suffix,
  .button--circle .button__caret {
    display: none;
  }

  /*
   * Caret modifier
   */

  .button--caret .button__suffix {
    display: none;
  }

  .button--caret .button__caret {
    height: auto;
  }

  /*
   * Loading modifier
   */

  .button--loading {
    position: relative;
    cursor: wait;
  }

  .button--loading .button__prefix,
  .button--loading .button__label,
  .button--loading .button__suffix,
  .button--loading .button__caret {
    visibility: hidden;
  }

  .button--loading sl-spinner {
    --indicator-color: currentColor;
    position: absolute;
    font-size: 1em;
    height: 1em;
    width: 1em;
    top: calc(50% - 0.5em);
    left: calc(50% - 0.5em);
  }

  /*
   * Badges
   */

  .button ::slotted(sl-badge) {
    position: absolute;
    top: 0;
    right: 0;
    translate: 50% -50%;
    pointer-events: none;
  }

  .button--rtl ::slotted(sl-badge) {
    right: auto;
    left: 0;
    translate: -50% -50%;
  }

  /*
   * Button spacing
   */

  .button--has-label.button--small .button__label {
    padding: 0 var(--sl-spacing-small);
  }

  .button--has-label.button--medium .button__label {
    padding: 0 var(--sl-spacing-medium);
  }

  .button--has-label.button--large .button__label {
    padding: 0 var(--sl-spacing-large);
  }

  .button--has-prefix.button--small {
    padding-inline-start: var(--sl-spacing-x-small);
  }

  .button--has-prefix.button--small .button__label {
    padding-inline-start: var(--sl-spacing-x-small);
  }

  .button--has-prefix.button--medium {
    padding-inline-start: var(--sl-spacing-small);
  }

  .button--has-prefix.button--medium .button__label {
    padding-inline-start: var(--sl-spacing-small);
  }

  .button--has-prefix.button--large {
    padding-inline-start: var(--sl-spacing-small);
  }

  .button--has-prefix.button--large .button__label {
    padding-inline-start: var(--sl-spacing-small);
  }

  .button--has-suffix.button--small,
  .button--caret.button--small {
    padding-inline-end: var(--sl-spacing-x-small);
  }

  .button--has-suffix.button--small .button__label,
  .button--caret.button--small .button__label {
    padding-inline-end: var(--sl-spacing-x-small);
  }

  .button--has-suffix.button--medium,
  .button--caret.button--medium {
    padding-inline-end: var(--sl-spacing-small);
  }

  .button--has-suffix.button--medium .button__label,
  .button--caret.button--medium .button__label {
    padding-inline-end: var(--sl-spacing-small);
  }

  .button--has-suffix.button--large,
  .button--caret.button--large {
    padding-inline-end: var(--sl-spacing-small);
  }

  .button--has-suffix.button--large .button__label,
  .button--caret.button--large .button__label {
    padding-inline-end: var(--sl-spacing-small);
  }

  /*
   * Button groups support a variety of button types (e.g. buttons with tooltips, buttons as dropdown triggers, etc.).
   * This means buttons aren't always direct descendants of the button group, thus we can't target them with the
   * ::slotted selector. To work around this, the button group component does some magic to add these special classes to
   * buttons and we style them here instead.
   */

  :host([data-sl-button-group__button--first]:not([data-sl-button-group__button--last])) .button {
    border-start-end-radius: 0;
    border-end-end-radius: 0;
  }

  :host([data-sl-button-group__button--inner]) .button {
    border-radius: 0;
  }

  :host([data-sl-button-group__button--last]:not([data-sl-button-group__button--first])) .button {
    border-start-start-radius: 0;
    border-end-start-radius: 0;
  }

  /* All except the first */
  :host([data-sl-button-group__button]:not([data-sl-button-group__button--first])) {
    margin-inline-start: calc(-1 * var(--sl-input-border-width));
  }

  /* Add a visual separator between solid buttons */
  :host(
      [data-sl-button-group__button]:not(
          [data-sl-button-group__button--first],
          [data-sl-button-group__button--radio],
          [variant='default']
        ):not(:hover)
    )
    .button:after {
    content: '';
    position: absolute;
    top: 0;
    inset-inline-start: 0;
    bottom: 0;
    border-left: solid 1px rgb(128 128 128 / 33%);
    mix-blend-mode: multiply;
  }

  /* Bump hovered, focused, and checked buttons up so their focus ring isn't clipped */
  :host([data-sl-button-group__button--hover]) {
    z-index: 1;
  }

  /* Focus and checked are always on top */
  :host([data-sl-button-group__button--focus]),
  :host([data-sl-button-group__button][checked]) {
    z-index: 2;
  }
`;var lr=Symbol.for(""),pi=t=>{if(t?.r===lr)return t?._$litStatic$},Rt=(t,...e)=>({_$litStatic$:e.reduce((o,r,i)=>o+(s=>{if(s._$litStatic$!==void 0)return s._$litStatic$;throw Error(`Value passed to 'literal' function must be a 'literal' result: ${s}. Use 'unsafeStatic' to pass non-literal values, but
            take care to ensure page security.`)})(r)+t[i+1],t[0]),r:lr}),nr=new Map,Ue=t=>(e,...o)=>{let r=o.length,i,s,n=[],l=[],c,d=0,p=!1;for(;d<r;){for(c=e[d];d<r&&(s=o[d],(i=pi(s))!==void 0);)c+=i+e[++d],p=!0;d!==r&&l.push(s),n.push(c),d++}if(d===r&&n.push(e[r]),p){let h=n.join("$$lit$$");(e=nr.get(h))===void 0&&(n.raw=n,nr.set(h,e=n)),o=l}return t(e,...o)},Bt=Ue(k),tn=Ue(zo),en=Ue(To);var T=t=>t??z;var fe=class{constructor(t,...e){this.slotNames=[],this.handleSlotChange=o=>{let r=o.target;(this.slotNames.includes("[default]")&&!r.name||r.name&&this.slotNames.includes(r.name))&&this.host.requestUpdate()},(this.host=t).addController(this),this.slotNames=e}hasDefaultSlot(){return[...this.host.childNodes].some(t=>{if(t.nodeType===t.TEXT_NODE&&t.textContent.trim()!=="")return!0;if(t.nodeType===t.ELEMENT_NODE){let e=t;if(e.tagName.toLowerCase()==="sl-visually-hidden")return!1;if(!e.hasAttribute("slot"))return!0}return!1})}hasNamedSlot(t){return this.host.querySelector(`:scope > [slot="${t}"]`)!==null}test(t){return t==="[default]"?this.hasDefaultSlot():this.hasNamedSlot(t)}hostConnected(){this.host.shadowRoot.addEventListener("slotchange",this.handleSlotChange)}hostDisconnected(){this.host.shadowRoot.removeEventListener("slotchange",this.handleSlotChange)}};var{I:cn}=Fo,ar=(t,e)=>e===void 0?t?._$litType$!==void 0:t?._$litType$===e;var We="";function cr(t){We=t}function ur(t=""){if(!We){let e=[...document.getElementsByTagName("script")],o=e.find(r=>r.hasAttribute("data-shoelace"));if(o)cr(o.getAttribute("data-shoelace"));else{let r=e.find(s=>/shoelace(\.min)?\.js($|\?)/.test(s.src)||/shoelace-autoloader(\.min)?\.js($|\?)/.test(s.src)),i="";r&&(i=r.getAttribute("src")),cr(i.split("/").slice(0,-1).join("/"))}}return We.replace(/\/$/,"")+(t?`/${t.replace(/^\//,"")}`:"")}var fi={name:"default",resolver:t=>ur(`assets/icons/${t}.svg`)},dr=fi;var hr={caret:`
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <polyline points="6 9 12 15 18 9"></polyline>
    </svg>
  `,check:`
    <svg part="checked-icon" class="checkbox__icon" viewBox="0 0 16 16">
      <g stroke="none" stroke-width="1" fill="none" fill-rule="evenodd" stroke-linecap="round">
        <g stroke="currentColor">
          <g transform="translate(3.428571, 3.428571)">
            <path d="M0,5.71428571 L3.42857143,9.14285714"></path>
            <path d="M9.14285714,0 L3.42857143,9.14285714"></path>
          </g>
        </g>
      </g>
    </svg>
  `,"chevron-down":`
    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="currentColor" class="bi bi-chevron-down" viewBox="0 0 16 16">
      <path fill-rule="evenodd" d="M1.646 4.646a.5.5 0 0 1 .708 0L8 10.293l5.646-5.647a.5.5 0 0 1 .708.708l-6 6a.5.5 0 0 1-.708 0l-6-6a.5.5 0 0 1 0-.708z"/>
    </svg>
  `,"chevron-left":`
    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="currentColor" class="bi bi-chevron-left" viewBox="0 0 16 16">
      <path fill-rule="evenodd" d="M11.354 1.646a.5.5 0 0 1 0 .708L5.707 8l5.647 5.646a.5.5 0 0 1-.708.708l-6-6a.5.5 0 0 1 0-.708l6-6a.5.5 0 0 1 .708 0z"/>
    </svg>
  `,"chevron-right":`
    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="currentColor" class="bi bi-chevron-right" viewBox="0 0 16 16">
      <path fill-rule="evenodd" d="M4.646 1.646a.5.5 0 0 1 .708 0l6 6a.5.5 0 0 1 0 .708l-6 6a.5.5 0 0 1-.708-.708L10.293 8 4.646 2.354a.5.5 0 0 1 0-.708z"/>
    </svg>
  `,copy:`
    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="currentColor" class="bi bi-copy" viewBox="0 0 16 16">
      <path fill-rule="evenodd" d="M4 2a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V2Zm2-1a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V2a1 1 0 0 0-1-1H6ZM2 5a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1v-1h1v1a2 2 0 0 1-2 2H2a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h1v1H2Z"/>
    </svg>
  `,eye:`
    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="currentColor" class="bi bi-eye" viewBox="0 0 16 16">
      <path d="M16 8s-3-5.5-8-5.5S0 8 0 8s3 5.5 8 5.5S16 8 16 8zM1.173 8a13.133 13.133 0 0 1 1.66-2.043C4.12 4.668 5.88 3.5 8 3.5c2.12 0 3.879 1.168 5.168 2.457A13.133 13.133 0 0 1 14.828 8c-.058.087-.122.183-.195.288-.335.48-.83 1.12-1.465 1.755C11.879 11.332 10.119 12.5 8 12.5c-2.12 0-3.879-1.168-5.168-2.457A13.134 13.134 0 0 1 1.172 8z"/>
      <path d="M8 5.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5zM4.5 8a3.5 3.5 0 1 1 7 0 3.5 3.5 0 0 1-7 0z"/>
    </svg>
  `,"eye-slash":`
    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="currentColor" class="bi bi-eye-slash" viewBox="0 0 16 16">
      <path d="M13.359 11.238C15.06 9.72 16 8 16 8s-3-5.5-8-5.5a7.028 7.028 0 0 0-2.79.588l.77.771A5.944 5.944 0 0 1 8 3.5c2.12 0 3.879 1.168 5.168 2.457A13.134 13.134 0 0 1 14.828 8c-.058.087-.122.183-.195.288-.335.48-.83 1.12-1.465 1.755-.165.165-.337.328-.517.486l.708.709z"/>
      <path d="M11.297 9.176a3.5 3.5 0 0 0-4.474-4.474l.823.823a2.5 2.5 0 0 1 2.829 2.829l.822.822zm-2.943 1.299.822.822a3.5 3.5 0 0 1-4.474-4.474l.823.823a2.5 2.5 0 0 0 2.829 2.829z"/>
      <path d="M3.35 5.47c-.18.16-.353.322-.518.487A13.134 13.134 0 0 0 1.172 8l.195.288c.335.48.83 1.12 1.465 1.755C4.121 11.332 5.881 12.5 8 12.5c.716 0 1.39-.133 2.02-.36l.77.772A7.029 7.029 0 0 1 8 13.5C3 13.5 0 8 0 8s.939-1.721 2.641-3.238l.708.709zm10.296 8.884-12-12 .708-.708 12 12-.708.708z"/>
    </svg>
  `,eyedropper:`
    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="currentColor" class="bi bi-eyedropper" viewBox="0 0 16 16">
      <path d="M13.354.646a1.207 1.207 0 0 0-1.708 0L8.5 3.793l-.646-.647a.5.5 0 1 0-.708.708L8.293 5l-7.147 7.146A.5.5 0 0 0 1 12.5v1.793l-.854.853a.5.5 0 1 0 .708.707L1.707 15H3.5a.5.5 0 0 0 .354-.146L11 7.707l1.146 1.147a.5.5 0 0 0 .708-.708l-.647-.646 3.147-3.146a1.207 1.207 0 0 0 0-1.708l-2-2zM2 12.707l7-7L10.293 7l-7 7H2v-1.293z"></path>
    </svg>
  `,"grip-vertical":`
    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="currentColor" class="bi bi-grip-vertical" viewBox="0 0 16 16">
      <path d="M7 2a1 1 0 1 1-2 0 1 1 0 0 1 2 0zm3 0a1 1 0 1 1-2 0 1 1 0 0 1 2 0zM7 5a1 1 0 1 1-2 0 1 1 0 0 1 2 0zm3 0a1 1 0 1 1-2 0 1 1 0 0 1 2 0zM7 8a1 1 0 1 1-2 0 1 1 0 0 1 2 0zm3 0a1 1 0 1 1-2 0 1 1 0 0 1 2 0zm-3 3a1 1 0 1 1-2 0 1 1 0 0 1 2 0zm3 0a1 1 0 1 1-2 0 1 1 0 0 1 2 0zm-3 3a1 1 0 1 1-2 0 1 1 0 0 1 2 0zm3 0a1 1 0 1 1-2 0 1 1 0 0 1 2 0z"></path>
    </svg>
  `,indeterminate:`
    <svg part="indeterminate-icon" class="checkbox__icon" viewBox="0 0 16 16">
      <g stroke="none" stroke-width="1" fill="none" fill-rule="evenodd" stroke-linecap="round">
        <g stroke="currentColor" stroke-width="2">
          <g transform="translate(2.285714, 6.857143)">
            <path d="M10.2857143,1.14285714 L1.14285714,1.14285714"></path>
          </g>
        </g>
      </g>
    </svg>
  `,"person-fill":`
    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="currentColor" class="bi bi-person-fill" viewBox="0 0 16 16">
      <path d="M3 14s-1 0-1-1 1-4 6-4 6 3 6 4-1 1-1 1H3zm5-6a3 3 0 1 0 0-6 3 3 0 0 0 0 6z"/>
    </svg>
  `,"play-fill":`
    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="currentColor" class="bi bi-play-fill" viewBox="0 0 16 16">
      <path d="m11.596 8.697-6.363 3.692c-.54.313-1.233-.066-1.233-.697V4.308c0-.63.692-1.01 1.233-.696l6.363 3.692a.802.802 0 0 1 0 1.393z"></path>
    </svg>
  `,"pause-fill":`
    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="currentColor" class="bi bi-pause-fill" viewBox="0 0 16 16">
      <path d="M5.5 3.5A1.5 1.5 0 0 1 7 5v6a1.5 1.5 0 0 1-3 0V5a1.5 1.5 0 0 1 1.5-1.5zm5 0A1.5 1.5 0 0 1 12 5v6a1.5 1.5 0 0 1-3 0V5a1.5 1.5 0 0 1 1.5-1.5z"></path>
    </svg>
  `,radio:`
    <svg part="checked-icon" class="radio__icon" viewBox="0 0 16 16">
      <g stroke="none" stroke-width="1" fill="none" fill-rule="evenodd">
        <g fill="currentColor">
          <circle cx="8" cy="8" r="3.42857143"></circle>
        </g>
      </g>
    </svg>
  `,"star-fill":`
    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="currentColor" class="bi bi-star-fill" viewBox="0 0 16 16">
      <path d="M3.612 15.443c-.386.198-.824-.149-.746-.592l.83-4.73L.173 6.765c-.329-.314-.158-.888.283-.95l4.898-.696L7.538.792c.197-.39.73-.39.927 0l2.184 4.327 4.898.696c.441.062.612.636.282.95l-3.522 3.356.83 4.73c.078.443-.36.79-.746.592L8 13.187l-4.389 2.256z"/>
    </svg>
  `,"x-lg":`
    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="currentColor" class="bi bi-x-lg" viewBox="0 0 16 16">
      <path d="M2.146 2.854a.5.5 0 1 1 .708-.708L8 7.293l5.146-5.147a.5.5 0 0 1 .708.708L8.707 8l5.147 5.146a.5.5 0 0 1-.708.708L8 8.707l-5.146 5.147a.5.5 0 0 1-.708-.708L7.293 8 2.146 2.854Z"/>
    </svg>
  `,"x-circle-fill":`
    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="currentColor" class="bi bi-x-circle-fill" viewBox="0 0 16 16">
      <path d="M16 8A8 8 0 1 1 0 8a8 8 0 0 1 16 0zM5.354 4.646a.5.5 0 1 0-.708.708L7.293 8l-2.647 2.646a.5.5 0 0 0 .708.708L8 8.707l2.646 2.647a.5.5 0 0 0 .708-.708L8.707 8l2.647-2.646a.5.5 0 0 0-.708-.708L8 7.293 5.354 4.646z"></path>
    </svg>
  `},mi={name:"system",resolver:t=>t in hr?`data:image/svg+xml,${encodeURIComponent(hr[t])}`:""},pr=mi;var gi=[dr,pr],je=[];function fr(t){je.push(t)}function mr(t){je=je.filter(e=>e!==t)}function qe(t){return gi.find(e=>e.name===t)}var gr=A`
  :host {
    display: inline-block;
    width: 1em;
    height: 1em;
    box-sizing: content-box !important;
  }

  svg {
    display: block;
    height: 100%;
    width: 100%;
  }
`;function M(t,e){let o=S({waitUntilFirstUpdate:!1},e);return(r,i)=>{let{update:s}=r,n=Array.isArray(t)?t:[t];r.update=function(l){n.forEach(c=>{let d=c;if(l.has(d)){let p=l.get(d),h=this[d];p!==h&&(!o.waitUntilFirstUpdate||this.hasUpdated)&&this[i](p,h)}}),s.call(this,l)}}}var Jt=Symbol(),me=Symbol(),Ke,Ye=new Map,H=class extends E{constructor(){super(...arguments),this.initialRender=!1,this.svg=null,this.label="",this.library="default"}async resolveIcon(t,e){var o;let r;if(e?.spriteSheet)return this.svg=k`<svg part="svg">
        <use part="use" href="${t}"></use>
      </svg>`,this.svg;try{if(r=await fetch(t,{mode:"cors"}),!r.ok)return r.status===410?Jt:me}catch{return me}try{let i=document.createElement("div");i.innerHTML=await r.text();let s=i.firstElementChild;if(((o=s?.tagName)==null?void 0:o.toLowerCase())!=="svg")return Jt;Ke||(Ke=new DOMParser);let l=Ke.parseFromString(s.outerHTML,"text/html").body.querySelector("svg");return l?(l.part.add("svg"),document.adoptNode(l)):Jt}catch{return Jt}}connectedCallback(){super.connectedCallback(),fr(this)}firstUpdated(){this.initialRender=!0,this.setIcon()}disconnectedCallback(){super.disconnectedCallback(),mr(this)}getIconSource(){let t=qe(this.library);return this.name&&t?{url:t.resolver(this.name),fromLibrary:!0}:{url:this.src,fromLibrary:!1}}handleLabelChange(){typeof this.label=="string"&&this.label.length>0?(this.setAttribute("role","img"),this.setAttribute("aria-label",this.label),this.removeAttribute("aria-hidden")):(this.removeAttribute("role"),this.removeAttribute("aria-label"),this.setAttribute("aria-hidden","true"))}async setIcon(){var t;let{url:e,fromLibrary:o}=this.getIconSource(),r=o?qe(this.library):void 0;if(!e){this.svg=null;return}let i=Ye.get(e);if(i||(i=this.resolveIcon(e,r),Ye.set(e,i)),!this.initialRender)return;let s=await i;if(s===me&&Ye.delete(e),e===this.getIconSource().url){if(ar(s)){if(this.svg=s,r){await this.updateComplete;let n=this.shadowRoot.querySelector("[part='svg']");typeof r.mutator=="function"&&n&&r.mutator(n)}return}switch(s){case me:case Jt:this.svg=null,this.emit("sl-error");break;default:this.svg=s.cloneNode(!0),(t=r?.mutator)==null||t.call(r,this.svg),this.emit("sl-load")}}}render(){return this.svg}};H.styles=[D,gr];a([R()],H.prototype,"svg",2);a([u({reflect:!0})],H.prototype,"name",2);a([u()],H.prototype,"src",2);a([u()],H.prototype,"label",2);a([u({reflect:!0})],H.prototype,"library",2);a([M("label")],H.prototype,"handleLabelChange",1);a([M(["name","src","library"])],H.prototype,"setIcon",1);var ge={ATTRIBUTE:1,CHILD:2,PROPERTY:3,BOOLEAN_ATTRIBUTE:4,EVENT:5,ELEMENT:6},be=t=>(...e)=>({_$litDirective$:t,values:e}),ve=class{constructor(t){}get _$AU(){return this._$AM._$AU}_$AT(t,e,o){this._$Ct=t,this._$AM=e,this._$Ci=o}_$AS(t,e){return this.update(t,e)}update(t,e){return this.render(...e)}};var V=be(class extends ve{constructor(t){var e;if(super(t),t.type!==ge.ATTRIBUTE||t.name!=="class"||((e=t.strings)==null?void 0:e.length)>2)throw Error("`classMap()` can only be used in the `class` attribute and must be the only part in the attribute.")}render(t){return" "+Object.keys(t).filter(e=>t[e]).join(" ")+" "}update(t,[e]){var o,r;if(this.st===void 0){this.st=new Set,t.strings!==void 0&&(this.nt=new Set(t.strings.join(" ").split(/\s/).filter(s=>s!=="")));for(let s in e)e[s]&&!((o=this.nt)!=null&&o.has(s))&&this.st.add(s);return this.render(e)}let i=t.element.classList;for(let s of this.st)s in e||(i.remove(s),this.st.delete(s));for(let s in e){let n=!!e[s];n===this.st.has(s)||(r=this.nt)!=null&&r.has(s)||(n?(i.add(s),this.st.add(s)):(i.remove(s),this.st.delete(s)))}return rt}});var C=class extends E{constructor(){super(...arguments),this.formControlController=new he(this,{assumeInteractionOn:["click"]}),this.hasSlotController=new fe(this,"[default]","prefix","suffix"),this.localize=new F(this),this.hasFocus=!1,this.invalid=!1,this.title="",this.variant="default",this.size="medium",this.caret=!1,this.disabled=!1,this.loading=!1,this.outline=!1,this.pill=!1,this.circle=!1,this.type="button",this.name="",this.value="",this.href="",this.rel="noreferrer noopener"}get validity(){return this.isButton()?this.button.validity:pe}get validationMessage(){return this.isButton()?this.button.validationMessage:""}firstUpdated(){this.isButton()&&this.formControlController.updateValidity()}handleBlur(){this.hasFocus=!1,this.emit("sl-blur")}handleFocus(){this.hasFocus=!0,this.emit("sl-focus")}handleClick(){this.type==="submit"&&this.formControlController.submit(this),this.type==="reset"&&this.formControlController.reset(this)}handleInvalid(t){this.formControlController.setValidity(!1),this.formControlController.emitInvalidEvent(t)}isButton(){return!this.href}isLink(){return!!this.href}handleDisabledChange(){this.isButton()&&this.formControlController.setValidity(this.disabled)}click(){this.button.click()}focus(t){this.button.focus(t)}blur(){this.button.blur()}checkValidity(){return this.isButton()?this.button.checkValidity():!0}getForm(){return this.formControlController.getForm()}reportValidity(){return this.isButton()?this.button.reportValidity():!0}setCustomValidity(t){this.isButton()&&(this.button.setCustomValidity(t),this.formControlController.updateValidity())}render(){let t=this.isLink(),e=t?Rt`a`:Rt`button`;return Bt`
      <${e}
        part="base"
        class=${V({button:!0,"button--default":this.variant==="default","button--primary":this.variant==="primary","button--success":this.variant==="success","button--neutral":this.variant==="neutral","button--warning":this.variant==="warning","button--danger":this.variant==="danger","button--text":this.variant==="text","button--small":this.size==="small","button--medium":this.size==="medium","button--large":this.size==="large","button--caret":this.caret,"button--circle":this.circle,"button--disabled":this.disabled,"button--focused":this.hasFocus,"button--loading":this.loading,"button--standard":!this.outline,"button--outline":this.outline,"button--pill":this.pill,"button--rtl":this.localize.dir()==="rtl","button--has-label":this.hasSlotController.test("[default]"),"button--has-prefix":this.hasSlotController.test("prefix"),"button--has-suffix":this.hasSlotController.test("suffix")})}
        ?disabled=${T(t?void 0:this.disabled)}
        type=${T(t?void 0:this.type)}
        title=${this.title}
        name=${T(t?void 0:this.name)}
        value=${T(t?void 0:this.value)}
        href=${T(t&&!this.disabled?this.href:void 0)}
        target=${T(t?this.target:void 0)}
        download=${T(t?this.download:void 0)}
        rel=${T(t?this.rel:void 0)}
        role=${T(t?void 0:"button")}
        aria-disabled=${this.disabled?"true":"false"}
        tabindex=${this.disabled?"-1":"0"}
        @blur=${this.handleBlur}
        @focus=${this.handleFocus}
        @invalid=${this.isButton()?this.handleInvalid:null}
        @click=${this.handleClick}
      >
        <slot name="prefix" part="prefix" class="button__prefix"></slot>
        <slot part="label" class="button__label"></slot>
        <slot name="suffix" part="suffix" class="button__suffix"></slot>
        ${this.caret?Bt` <sl-icon part="caret" class="button__caret" library="system" name="caret"></sl-icon> `:""}
        ${this.loading?Bt`<sl-spinner part="spinner"></sl-spinner>`:""}
      </${e}>
    `}};C.styles=[D,sr];C.dependencies={"sl-icon":H,"sl-spinner":Fe};a([O(".button")],C.prototype,"button",2);a([R()],C.prototype,"hasFocus",2);a([R()],C.prototype,"invalid",2);a([u()],C.prototype,"title",2);a([u({reflect:!0})],C.prototype,"variant",2);a([u({reflect:!0})],C.prototype,"size",2);a([u({type:Boolean,reflect:!0})],C.prototype,"caret",2);a([u({type:Boolean,reflect:!0})],C.prototype,"disabled",2);a([u({type:Boolean,reflect:!0})],C.prototype,"loading",2);a([u({type:Boolean,reflect:!0})],C.prototype,"outline",2);a([u({type:Boolean,reflect:!0})],C.prototype,"pill",2);a([u({type:Boolean,reflect:!0})],C.prototype,"circle",2);a([u()],C.prototype,"type",2);a([u()],C.prototype,"name",2);a([u()],C.prototype,"value",2);a([u()],C.prototype,"href",2);a([u()],C.prototype,"target",2);a([u()],C.prototype,"rel",2);a([u()],C.prototype,"download",2);a([u()],C.prototype,"form",2);a([u({attribute:"formaction"})],C.prototype,"formAction",2);a([u({attribute:"formenctype"})],C.prototype,"formEnctype",2);a([u({attribute:"formmethod"})],C.prototype,"formMethod",2);a([u({attribute:"formnovalidate",type:Boolean})],C.prototype,"formNoValidate",2);a([u({attribute:"formtarget"})],C.prototype,"formTarget",2);a([M("disabled",{waitUntilFirstUpdate:!0})],C.prototype,"handleDisabledChange",1);C.define("sl-button");var br=A`
  :host {
    display: inline-block;
  }

  .button-group {
    display: flex;
    flex-wrap: nowrap;
  }
`;var Mt=class extends E{constructor(){super(...arguments),this.disableRole=!1,this.label=""}handleFocus(t){let e=Qt(t.target);e?.toggleAttribute("data-sl-button-group__button--focus",!0)}handleBlur(t){let e=Qt(t.target);e?.toggleAttribute("data-sl-button-group__button--focus",!1)}handleMouseOver(t){let e=Qt(t.target);e?.toggleAttribute("data-sl-button-group__button--hover",!0)}handleMouseOut(t){let e=Qt(t.target);e?.toggleAttribute("data-sl-button-group__button--hover",!1)}handleSlotChange(){let t=[...this.defaultSlot.assignedElements({flatten:!0})];t.forEach(e=>{let o=t.indexOf(e),r=Qt(e);r&&(r.toggleAttribute("data-sl-button-group__button",!0),r.toggleAttribute("data-sl-button-group__button--first",o===0),r.toggleAttribute("data-sl-button-group__button--inner",o>0&&o<t.length-1),r.toggleAttribute("data-sl-button-group__button--last",o===t.length-1),r.toggleAttribute("data-sl-button-group__button--radio",r.tagName.toLowerCase()==="sl-radio-button"))})}render(){return k`
      <div
        part="base"
        class="button-group"
        role="${this.disableRole?"presentation":"group"}"
        aria-label=${this.label}
        @focusout=${this.handleBlur}
        @focusin=${this.handleFocus}
        @mouseover=${this.handleMouseOver}
        @mouseout=${this.handleMouseOut}
      >
        <slot @slotchange=${this.handleSlotChange}></slot>
      </div>
    `}};Mt.styles=[D,br];a([O("slot")],Mt.prototype,"defaultSlot",2);a([R()],Mt.prototype,"disableRole",2);a([u()],Mt.prototype,"label",2);function Qt(t){var e;let o="sl-button, sl-radio-button";return(e=t.closest(o))!=null?e:t.querySelector(o)}Mt.define("sl-button-group");var vr=A`
  :host {
    display: inline-block;
  }

  .dropdown::part(popup) {
    z-index: var(--sl-z-index-dropdown);
  }

  .dropdown[data-current-placement^='top']::part(popup) {
    transform-origin: bottom;
  }

  .dropdown[data-current-placement^='bottom']::part(popup) {
    transform-origin: top;
  }

  .dropdown[data-current-placement^='left']::part(popup) {
    transform-origin: right;
  }

  .dropdown[data-current-placement^='right']::part(popup) {
    transform-origin: left;
  }

  .dropdown__trigger {
    display: block;
  }

  .dropdown__panel {
    font-family: var(--sl-font-sans);
    font-size: var(--sl-font-size-medium);
    font-weight: var(--sl-font-weight-normal);
    box-shadow: var(--sl-shadow-large);
    border-radius: var(--sl-border-radius-medium);
    pointer-events: none;
  }

  .dropdown--open .dropdown__panel {
    display: block;
    pointer-events: all;
  }

  /* When users slot a menu, make sure it conforms to the popup's auto-size */
  ::slotted(sl-menu) {
    max-width: var(--auto-size-available-width) !important;
    max-height: var(--auto-size-available-height) !important;
  }
`;function*_r(t=document.activeElement){t!=null&&(yield t,"shadowRoot"in t&&t.shadowRoot&&t.shadowRoot.mode!=="closed"&&(yield*rr(_r(t.shadowRoot.activeElement))))}function wr(){return[..._r()].pop()}var yr=new WeakMap;function xr(t){let e=yr.get(t);return e||(e=window.getComputedStyle(t,null),yr.set(t,e)),e}function bi(t){if(typeof t.checkVisibility=="function")return t.checkVisibility({checkOpacity:!1,checkVisibilityCSS:!0});let e=xr(t);return e.visibility!=="hidden"&&e.display!=="none"}function vi(t){let e=xr(t),{overflowY:o,overflowX:r}=e;return o==="scroll"||r==="scroll"?!0:o!=="auto"||r!=="auto"?!1:t.scrollHeight>t.clientHeight&&o==="auto"||t.scrollWidth>t.clientWidth&&r==="auto"}function yi(t){let e=t.tagName.toLowerCase(),o=Number(t.getAttribute("tabindex"));if(t.hasAttribute("tabindex")&&(isNaN(o)||o<=-1)||t.hasAttribute("disabled")||t.closest("[inert]"))return!1;if(e==="input"&&t.getAttribute("type")==="radio"){let s=t.getRootNode(),n=`input[type='radio'][name="${t.getAttribute("name")}"]`,l=s.querySelector(`${n}:checked`);return l?l===t:s.querySelector(n)===t}return bi(t)?(e==="audio"||e==="video")&&t.hasAttribute("controls")||t.hasAttribute("tabindex")||t.hasAttribute("contenteditable")&&t.getAttribute("contenteditable")!=="false"||["button","input","select","textarea","a","audio","video","summary","iframe"].includes(e)?!0:vi(t):!1}function Cr(t){var e,o;let r=wi(t),i=(e=r[0])!=null?e:null,s=(o=r[r.length-1])!=null?o:null;return{start:i,end:s}}function _i(t,e){var o;return((o=t.getRootNode({composed:!0}))==null?void 0:o.host)!==e}function wi(t){let e=new WeakMap,o=[];function r(i){if(i instanceof Element){if(i.hasAttribute("inert")||i.closest("[inert]")||e.has(i))return;e.set(i,!0),!o.includes(i)&&yi(i)&&o.push(i),i instanceof HTMLSlotElement&&_i(i,t)&&i.assignedElements({flatten:!0}).forEach(s=>{r(s)}),i.shadowRoot!==null&&i.shadowRoot.mode==="open"&&r(i.shadowRoot)}for(let s of i.children)r(s)}return r(t),o.sort((i,s)=>{let n=Number(i.getAttribute("tabindex"))||0;return(Number(s.getAttribute("tabindex"))||0)-n})}var $r=A`
  :host {
    --arrow-color: var(--sl-color-neutral-1000);
    --arrow-size: 6px;

    /*
     * These properties are computed to account for the arrow's dimensions after being rotated 45º. The constant
     * 0.7071 is derived from sin(45), which is the diagonal size of the arrow's container after rotating.
     */
    --arrow-size-diagonal: calc(var(--arrow-size) * 0.7071);
    --arrow-padding-offset: calc(var(--arrow-size-diagonal) - var(--arrow-size));

    display: contents;
  }

  .popup {
    position: absolute;
    isolation: isolate;
    max-width: var(--auto-size-available-width, none);
    max-height: var(--auto-size-available-height, none);
  }

  .popup--fixed {
    position: fixed;
  }

  .popup:not(.popup--active) {
    display: none;
  }

  .popup__arrow {
    position: absolute;
    width: calc(var(--arrow-size-diagonal) * 2);
    height: calc(var(--arrow-size-diagonal) * 2);
    rotate: 45deg;
    background: var(--arrow-color);
    z-index: -1;
  }

  /* Hover bridge */
  .popup-hover-bridge:not(.popup-hover-bridge--visible) {
    display: none;
  }

  .popup-hover-bridge {
    position: fixed;
    z-index: calc(var(--sl-z-index-dropdown) - 1);
    top: 0;
    right: 0;
    bottom: 0;
    left: 0;
    clip-path: polygon(
      var(--hover-bridge-top-left-x, 0) var(--hover-bridge-top-left-y, 0),
      var(--hover-bridge-top-right-x, 0) var(--hover-bridge-top-right-y, 0),
      var(--hover-bridge-bottom-right-x, 0) var(--hover-bridge-bottom-right-y, 0),
      var(--hover-bridge-bottom-left-x, 0) var(--hover-bridge-bottom-left-y, 0)
    );
  }
`;var bt=Math.min,j=Math.max,we=Math.round,ye=Math.floor,it=t=>({x:t,y:t}),xi={left:"right",right:"left",bottom:"top",top:"bottom"},Ci={start:"end",end:"start"};function Ze(t,e,o){return j(t,bt(e,o))}function It(t,e){return typeof t=="function"?t(e):t}function vt(t){return t.split("-")[0]}function Ft(t){return t.split("-")[1]}function zr(t){return t==="x"?"y":"x"}function to(t){return t==="y"?"height":"width"}function At(t){return["top","bottom"].includes(vt(t))?"y":"x"}function eo(t){return zr(At(t))}function $i(t,e,o){o===void 0&&(o=!1);let r=Ft(t),i=eo(t),s=to(i),n=i==="x"?r===(o?"end":"start")?"right":"left":r==="start"?"bottom":"top";return e.reference[s]>e.floating[s]&&(n=xe(n)),[n,xe(n)]}function ki(t){let e=xe(t);return[Je(t),e,Je(e)]}function Je(t){return t.replace(/start|end/g,e=>Ci[e])}function Si(t,e,o){let r=["left","right"],i=["right","left"],s=["top","bottom"],n=["bottom","top"];switch(t){case"top":case"bottom":return o?e?i:r:e?r:i;case"left":case"right":return e?s:n;default:return[]}}function Ai(t,e,o,r){let i=Ft(t),s=Si(vt(t),o==="start",r);return i&&(s=s.map(n=>n+"-"+i),e&&(s=s.concat(s.map(Je)))),s}function xe(t){return t.replace(/left|right|bottom|top/g,e=>xi[e])}function Ei(t){return S({top:0,right:0,bottom:0,left:0},t)}function Tr(t){return typeof t!="number"?Ei(t):{top:t,right:t,bottom:t,left:t}}function Ce(t){let{x:e,y:o,width:r,height:i}=t;return{width:r,height:i,top:o,left:e,right:e+r,bottom:o+i,x:e,y:o}}function kr(t,e,o){let{reference:r,floating:i}=t,s=At(e),n=eo(e),l=to(n),c=vt(e),d=s==="y",p=r.x+r.width/2-i.width/2,h=r.y+r.height/2-i.height/2,m=r[l]/2-i[l]/2,f;switch(c){case"top":f={x:p,y:r.y-i.height};break;case"bottom":f={x:p,y:r.y+r.height};break;case"right":f={x:r.x+r.width,y:h};break;case"left":f={x:r.x-i.width,y:h};break;default:f={x:r.x,y:r.y}}switch(Ft(e)){case"start":f[n]-=m*(o&&d?-1:1);break;case"end":f[n]+=m*(o&&d?-1:1);break}return f}var Oi=async(t,e,o)=>{let{placement:r="bottom",strategy:i="absolute",middleware:s=[],platform:n}=o,l=s.filter(Boolean),c=await(n.isRTL==null?void 0:n.isRTL(e)),d=await n.getElementRects({reference:t,floating:e,strategy:i}),{x:p,y:h}=kr(d,r,c),m=r,f={},g=0;for(let b=0;b<l.length;b++){let{name:_,fn:v}=l[b],{x:w,y:$,data:L,reset:P}=await v({x:p,y:h,initialPlacement:r,placement:m,strategy:i,middlewareData:f,rects:d,platform:n,elements:{reference:t,floating:e}});p=w??p,h=$??h,f=B(S({},f),{[_]:S(S({},f[_]),L)}),P&&g<=50&&(g++,typeof P=="object"&&(P.placement&&(m=P.placement),P.rects&&(d=P.rects===!0?await n.getElementRects({reference:t,floating:e,strategy:i}):P.rects),{x:p,y:h}=kr(d,m,c)),b=-1)}return{x:p,y:h,placement:m,strategy:i,middlewareData:f}};async function oo(t,e){var o;e===void 0&&(e={});let{x:r,y:i,platform:s,rects:n,elements:l,strategy:c}=t,{boundary:d="clippingAncestors",rootBoundary:p="viewport",elementContext:h="floating",altBoundary:m=!1,padding:f=0}=It(e,t),g=Tr(f),_=l[m?h==="floating"?"reference":"floating":h],v=Ce(await s.getClippingRect({element:(o=await(s.isElement==null?void 0:s.isElement(_)))==null||o?_:_.contextElement||await(s.getDocumentElement==null?void 0:s.getDocumentElement(l.floating)),boundary:d,rootBoundary:p,strategy:c})),w=h==="floating"?{x:r,y:i,width:n.floating.width,height:n.floating.height}:n.reference,$=await(s.getOffsetParent==null?void 0:s.getOffsetParent(l.floating)),L=await(s.isElement==null?void 0:s.isElement($))?await(s.getScale==null?void 0:s.getScale($))||{x:1,y:1}:{x:1,y:1},P=Ce(s.convertOffsetParentRelativeRectToViewportRelativeRect?await s.convertOffsetParentRelativeRectToViewportRelativeRect({elements:l,rect:w,offsetParent:$,strategy:c}):w);return{top:(v.top-P.top+g.top)/L.y,bottom:(P.bottom-v.bottom+g.bottom)/L.y,left:(v.left-P.left+g.left)/L.x,right:(P.right-v.right+g.right)/L.x}}var zi=t=>({name:"arrow",options:t,async fn(e){let{x:o,y:r,placement:i,rects:s,platform:n,elements:l,middlewareData:c}=e,{element:d,padding:p=0}=It(t,e)||{};if(d==null)return{};let h=Tr(p),m={x:o,y:r},f=eo(i),g=to(f),b=await n.getDimensions(d),_=f==="y",v=_?"top":"left",w=_?"bottom":"right",$=_?"clientHeight":"clientWidth",L=s.reference[g]+s.reference[f]-m[f]-s.floating[g],P=m[f]-s.reference[f],Y=await(n.getOffsetParent==null?void 0:n.getOffsetParent(d)),Q=Y?Y[$]:0;(!Q||!await(n.isElement==null?void 0:n.isElement(Y)))&&(Q=l.floating[$]||s.floating[g]);let wt=L/2-P/2,tt=Q/2-b[g]/2-1,et=bt(h[v],tt),X=bt(h[w],tt),ot=et,ft=Q-b[g]-X,W=Q/2-b[g]/2+wt,G=Ze(ot,W,ft),Ot=!c.arrow&&Ft(i)!=null&&W!==G&&s.reference[g]/2-(W<ot?et:X)-b[g]/2<0,lt=Ot?W<ot?W-ot:W-ft:0;return{[f]:m[f]+lt,data:S({[f]:G,centerOffset:W-G-lt},Ot&&{alignmentOffset:lt}),reset:Ot}}}),Ti=function(t){return t===void 0&&(t={}),{name:"flip",options:t,async fn(e){var o,r;let{placement:i,middlewareData:s,rects:n,initialPlacement:l,platform:c,elements:d}=e,p=It(t,e),{mainAxis:h=!0,crossAxis:m=!0,fallbackPlacements:f,fallbackStrategy:g="bestFit",fallbackAxisSideDirection:b="none",flipAlignment:_=!0}=p,v=ce(p,["mainAxis","crossAxis","fallbackPlacements","fallbackStrategy","fallbackAxisSideDirection","flipAlignment"]);if((o=s.arrow)!=null&&o.alignmentOffset)return{};let w=vt(i),$=At(l),L=vt(l)===l,P=await(c.isRTL==null?void 0:c.isRTL(d.floating)),Y=f||(L||!_?[xe(l)]:ki(l)),Q=b!=="none";!f&&Q&&Y.push(...Ai(l,_,b,P));let wt=[l,...Y],tt=await oo(e,v),et=[],X=((r=s.flip)==null?void 0:r.overflows)||[];if(h&&et.push(tt[w]),m){let G=$i(i,n,P);et.push(tt[G[0]],tt[G[1]])}if(X=[...X,{placement:i,overflows:et}],!et.every(G=>G<=0)){var ot,ft;let G=(((ot=s.flip)==null?void 0:ot.index)||0)+1,Ot=wt[G];if(Ot)return{data:{index:G,overflows:X},reset:{placement:Ot}};let lt=(ft=X.filter(zt=>zt.overflows[0]<=0).sort((zt,mt)=>zt.overflows[1]-mt.overflows[1])[0])==null?void 0:ft.placement;if(!lt)switch(g){case"bestFit":{var W;let zt=(W=X.filter(mt=>{if(Q){let gt=At(mt.placement);return gt===$||gt==="y"}return!0}).map(mt=>[mt.placement,mt.overflows.filter(gt=>gt>0).reduce((gt,Xr)=>gt+Xr,0)]).sort((mt,gt)=>mt[1]-gt[1])[0])==null?void 0:W[0];zt&&(lt=zt);break}case"initialPlacement":lt=l;break}if(i!==lt)return{reset:{placement:lt}}}return{}}}};async function Li(t,e){let{placement:o,platform:r,elements:i}=t,s=await(r.isRTL==null?void 0:r.isRTL(i.floating)),n=vt(o),l=Ft(o),c=At(o)==="y",d=["left","top"].includes(n)?-1:1,p=s&&c?-1:1,h=It(e,t),{mainAxis:m,crossAxis:f,alignmentAxis:g}=typeof h=="number"?{mainAxis:h,crossAxis:0,alignmentAxis:null}:{mainAxis:h.mainAxis||0,crossAxis:h.crossAxis||0,alignmentAxis:h.alignmentAxis};return l&&typeof g=="number"&&(f=l==="end"?g*-1:g),c?{x:f*p,y:m*d}:{x:m*d,y:f*p}}var Pi=function(t){return t===void 0&&(t=0),{name:"offset",options:t,async fn(e){var o,r;let{x:i,y:s,placement:n,middlewareData:l}=e,c=await Li(e,t);return n===((o=l.offset)==null?void 0:o.placement)&&(r=l.arrow)!=null&&r.alignmentOffset?{}:{x:i+c.x,y:s+c.y,data:B(S({},c),{placement:n})}}}},Di=function(t){return t===void 0&&(t={}),{name:"shift",options:t,async fn(e){let{x:o,y:r,placement:i}=e,s=It(t,e),{mainAxis:n=!0,crossAxis:l=!1,limiter:c={fn:v=>{let{x:w,y:$}=v;return{x:w,y:$}}}}=s,d=ce(s,["mainAxis","crossAxis","limiter"]),p={x:o,y:r},h=await oo(e,d),m=At(vt(i)),f=zr(m),g=p[f],b=p[m];if(n){let v=f==="y"?"top":"left",w=f==="y"?"bottom":"right",$=g+h[v],L=g-h[w];g=Ze($,g,L)}if(l){let v=m==="y"?"top":"left",w=m==="y"?"bottom":"right",$=b+h[v],L=b-h[w];b=Ze($,b,L)}let _=c.fn(B(S({},e),{[f]:g,[m]:b}));return B(S({},_),{data:{x:_.x-o,y:_.y-r,enabled:{[f]:n,[m]:l}}})}}},Ri=function(t){return t===void 0&&(t={}),{name:"size",options:t,async fn(e){var o,r;let{placement:i,rects:s,platform:n,elements:l}=e,c=It(t,e),{apply:d=()=>{}}=c,p=ce(c,["apply"]),h=await oo(e,p),m=vt(i),f=Ft(i),g=At(i)==="y",{width:b,height:_}=s.floating,v,w;m==="top"||m==="bottom"?(v=m,w=f===(await(n.isRTL==null?void 0:n.isRTL(l.floating))?"start":"end")?"left":"right"):(w=m,v=f==="end"?"top":"bottom");let $=_-h.top-h.bottom,L=b-h.left-h.right,P=bt(_-h[v],$),Y=bt(b-h[w],L),Q=!e.middlewareData.shift,wt=P,tt=Y;if((o=e.middlewareData.shift)!=null&&o.enabled.x&&(tt=L),(r=e.middlewareData.shift)!=null&&r.enabled.y&&(wt=$),Q&&!f){let X=j(h.left,0),ot=j(h.right,0),ft=j(h.top,0),W=j(h.bottom,0);g?tt=b-2*(X!==0||ot!==0?X+ot:j(h.left,h.right)):wt=_-2*(ft!==0||W!==0?ft+W:j(h.top,h.bottom))}await d(B(S({},e),{availableWidth:tt,availableHeight:wt}));let et=await n.getDimensions(l.floating);return b!==et.width||_!==et.height?{reset:{rects:!0}}:{}}}};function $e(){return typeof window<"u"}function Ht(t){return Lr(t)?(t.nodeName||"").toLowerCase():"#document"}function q(t){var e;return(t==null||(e=t.ownerDocument)==null?void 0:e.defaultView)||window}function nt(t){var e;return(e=(Lr(t)?t.ownerDocument:t.document)||window.document)==null?void 0:e.documentElement}function Lr(t){return $e()?t instanceof Node||t instanceof q(t).Node:!1}function Z(t){return $e()?t instanceof Element||t instanceof q(t).Element:!1}function st(t){return $e()?t instanceof HTMLElement||t instanceof q(t).HTMLElement:!1}function Sr(t){return!$e()||typeof ShadowRoot>"u"?!1:t instanceof ShadowRoot||t instanceof q(t).ShadowRoot}function ee(t){let{overflow:e,overflowX:o,overflowY:r,display:i}=J(t);return/auto|scroll|overlay|hidden|clip/.test(e+r+o)&&!["inline","contents"].includes(i)}function Bi(t){return["table","td","th"].includes(Ht(t))}function ke(t){return[":popover-open",":modal"].some(e=>{try{return t.matches(e)}catch{return!1}})}function Se(t){let e=ro(),o=Z(t)?J(t):t;return o.transform!=="none"||o.perspective!=="none"||(o.containerType?o.containerType!=="normal":!1)||!e&&(o.backdropFilter?o.backdropFilter!=="none":!1)||!e&&(o.filter?o.filter!=="none":!1)||["transform","perspective","filter"].some(r=>(o.willChange||"").includes(r))||["paint","layout","strict","content"].some(r=>(o.contain||"").includes(r))}function Mi(t){let e=yt(t);for(;st(e)&&!Nt(e);){if(Se(e))return e;if(ke(e))return null;e=yt(e)}return null}function ro(){return typeof CSS>"u"||!CSS.supports?!1:CSS.supports("-webkit-backdrop-filter","none")}function Nt(t){return["html","body","#document"].includes(Ht(t))}function J(t){return q(t).getComputedStyle(t)}function Ae(t){return Z(t)?{scrollLeft:t.scrollLeft,scrollTop:t.scrollTop}:{scrollLeft:t.scrollX,scrollTop:t.scrollY}}function yt(t){if(Ht(t)==="html")return t;let e=t.assignedSlot||t.parentNode||Sr(t)&&t.host||nt(t);return Sr(e)?e.host:e}function Pr(t){let e=yt(t);return Nt(e)?t.ownerDocument?t.ownerDocument.body:t.body:st(e)&&ee(e)?e:Pr(e)}function te(t,e,o){var r;e===void 0&&(e=[]),o===void 0&&(o=!0);let i=Pr(t),s=i===((r=t.ownerDocument)==null?void 0:r.body),n=q(i);if(s){let l=Qe(n);return e.concat(n,n.visualViewport||[],ee(i)?i:[],l&&o?te(l):[])}return e.concat(i,te(i,[],o))}function Qe(t){return t.parent&&Object.getPrototypeOf(t.parent)?t.frameElement:null}function Dr(t){let e=J(t),o=parseFloat(e.width)||0,r=parseFloat(e.height)||0,i=st(t),s=i?t.offsetWidth:o,n=i?t.offsetHeight:r,l=we(o)!==s||we(r)!==n;return l&&(o=s,r=n),{width:o,height:r,$:l}}function io(t){return Z(t)?t:t.contextElement}function Vt(t){let e=io(t);if(!st(e))return it(1);let o=e.getBoundingClientRect(),{width:r,height:i,$:s}=Dr(e),n=(s?we(o.width):o.width)/r,l=(s?we(o.height):o.height)/i;return(!n||!Number.isFinite(n))&&(n=1),(!l||!Number.isFinite(l))&&(l=1),{x:n,y:l}}var Vi=it(0);function Rr(t){let e=q(t);return!ro()||!e.visualViewport?Vi:{x:e.visualViewport.offsetLeft,y:e.visualViewport.offsetTop}}function Ni(t,e,o){return e===void 0&&(e=!1),!o||e&&o!==q(t)?!1:e}function Et(t,e,o,r){e===void 0&&(e=!1),o===void 0&&(o=!1);let i=t.getBoundingClientRect(),s=io(t),n=it(1);e&&(r?Z(r)&&(n=Vt(r)):n=Vt(t));let l=Ni(s,o,r)?Rr(s):it(0),c=(i.left+l.x)/n.x,d=(i.top+l.y)/n.y,p=i.width/n.x,h=i.height/n.y;if(s){let m=q(s),f=r&&Z(r)?q(r):r,g=m,b=Qe(g);for(;b&&r&&f!==g;){let _=Vt(b),v=b.getBoundingClientRect(),w=J(b),$=v.left+(b.clientLeft+parseFloat(w.paddingLeft))*_.x,L=v.top+(b.clientTop+parseFloat(w.paddingTop))*_.y;c*=_.x,d*=_.y,p*=_.x,h*=_.y,c+=$,d+=L,g=q(b),b=Qe(g)}}return Ce({width:p,height:h,x:c,y:d})}function so(t,e){let o=Ae(t).scrollLeft;return e?e.left+o:Et(nt(t)).left+o}function Br(t,e,o){o===void 0&&(o=!1);let r=t.getBoundingClientRect(),i=r.left+e.scrollLeft-(o?0:so(t,r)),s=r.top+e.scrollTop;return{x:i,y:s}}function Ii(t){let{elements:e,rect:o,offsetParent:r,strategy:i}=t,s=i==="fixed",n=nt(r),l=e?ke(e.floating):!1;if(r===n||l&&s)return o;let c={scrollLeft:0,scrollTop:0},d=it(1),p=it(0),h=st(r);if((h||!h&&!s)&&((Ht(r)!=="body"||ee(n))&&(c=Ae(r)),st(r))){let f=Et(r);d=Vt(r),p.x=f.x+r.clientLeft,p.y=f.y+r.clientTop}let m=n&&!h&&!s?Br(n,c,!0):it(0);return{width:o.width*d.x,height:o.height*d.y,x:o.x*d.x-c.scrollLeft*d.x+p.x+m.x,y:o.y*d.y-c.scrollTop*d.y+p.y+m.y}}function Fi(t){return Array.from(t.getClientRects())}function Hi(t){let e=nt(t),o=Ae(t),r=t.ownerDocument.body,i=j(e.scrollWidth,e.clientWidth,r.scrollWidth,r.clientWidth),s=j(e.scrollHeight,e.clientHeight,r.scrollHeight,r.clientHeight),n=-o.scrollLeft+so(t),l=-o.scrollTop;return J(r).direction==="rtl"&&(n+=j(e.clientWidth,r.clientWidth)-i),{width:i,height:s,x:n,y:l}}function Ui(t,e){let o=q(t),r=nt(t),i=o.visualViewport,s=r.clientWidth,n=r.clientHeight,l=0,c=0;if(i){s=i.width,n=i.height;let d=ro();(!d||d&&e==="fixed")&&(l=i.offsetLeft,c=i.offsetTop)}return{width:s,height:n,x:l,y:c}}function Wi(t,e){let o=Et(t,!0,e==="fixed"),r=o.top+t.clientTop,i=o.left+t.clientLeft,s=st(t)?Vt(t):it(1),n=t.clientWidth*s.x,l=t.clientHeight*s.y,c=i*s.x,d=r*s.y;return{width:n,height:l,x:c,y:d}}function Ar(t,e,o){let r;if(e==="viewport")r=Ui(t,o);else if(e==="document")r=Hi(nt(t));else if(Z(e))r=Wi(e,o);else{let i=Rr(t);r={x:e.x-i.x,y:e.y-i.y,width:e.width,height:e.height}}return Ce(r)}function Mr(t,e){let o=yt(t);return o===e||!Z(o)||Nt(o)?!1:J(o).position==="fixed"||Mr(o,e)}function ji(t,e){let o=e.get(t);if(o)return o;let r=te(t,[],!1).filter(l=>Z(l)&&Ht(l)!=="body"),i=null,s=J(t).position==="fixed",n=s?yt(t):t;for(;Z(n)&&!Nt(n);){let l=J(n),c=Se(n);!c&&l.position==="fixed"&&(i=null),(s?!c&&!i:!c&&l.position==="static"&&!!i&&["absolute","fixed"].includes(i.position)||ee(n)&&!c&&Mr(t,n))?r=r.filter(p=>p!==n):i=l,n=yt(n)}return e.set(t,r),r}function qi(t){let{element:e,boundary:o,rootBoundary:r,strategy:i}=t,n=[...o==="clippingAncestors"?ke(e)?[]:ji(e,this._c):[].concat(o),r],l=n[0],c=n.reduce((d,p)=>{let h=Ar(e,p,i);return d.top=j(h.top,d.top),d.right=bt(h.right,d.right),d.bottom=bt(h.bottom,d.bottom),d.left=j(h.left,d.left),d},Ar(e,l,i));return{width:c.right-c.left,height:c.bottom-c.top,x:c.left,y:c.top}}function Ki(t){let{width:e,height:o}=Dr(t);return{width:e,height:o}}function Yi(t,e,o){let r=st(e),i=nt(e),s=o==="fixed",n=Et(t,!0,s,e),l={scrollLeft:0,scrollTop:0},c=it(0);if(r||!r&&!s)if((Ht(e)!=="body"||ee(i))&&(l=Ae(e)),r){let m=Et(e,!0,s,e);c.x=m.x+e.clientLeft,c.y=m.y+e.clientTop}else i&&(c.x=so(i));let d=i&&!r&&!s?Br(i,l):it(0),p=n.left+l.scrollLeft-c.x-d.x,h=n.top+l.scrollTop-c.y-d.y;return{x:p,y:h,width:n.width,height:n.height}}function Xe(t){return J(t).position==="static"}function Er(t,e){if(!st(t)||J(t).position==="fixed")return null;if(e)return e(t);let o=t.offsetParent;return nt(t)===o&&(o=o.ownerDocument.body),o}function Vr(t,e){let o=q(t);if(ke(t))return o;if(!st(t)){let i=yt(t);for(;i&&!Nt(i);){if(Z(i)&&!Xe(i))return i;i=yt(i)}return o}let r=Er(t,e);for(;r&&Bi(r)&&Xe(r);)r=Er(r,e);return r&&Nt(r)&&Xe(r)&&!Se(r)?o:r||Mi(t)||o}var Xi=async function(t){let e=this.getOffsetParent||Vr,o=this.getDimensions,r=await o(t.floating);return{reference:Yi(t.reference,await e(t.floating),t.strategy),floating:{x:0,y:0,width:r.width,height:r.height}}};function Gi(t){return J(t).direction==="rtl"}var _e={convertOffsetParentRelativeRectToViewportRelativeRect:Ii,getDocumentElement:nt,getClippingRect:qi,getOffsetParent:Vr,getElementRects:Xi,getClientRects:Fi,getDimensions:Ki,getScale:Vt,isElement:Z,isRTL:Gi};function Zi(t,e){let o=null,r,i=nt(t);function s(){var l;clearTimeout(r),(l=o)==null||l.disconnect(),o=null}function n(l,c){l===void 0&&(l=!1),c===void 0&&(c=1),s();let{left:d,top:p,width:h,height:m}=t.getBoundingClientRect();if(l||e(),!h||!m)return;let f=ye(p),g=ye(i.clientWidth-(d+h)),b=ye(i.clientHeight-(p+m)),_=ye(d),w={rootMargin:-f+"px "+-g+"px "+-b+"px "+-_+"px",threshold:j(0,bt(1,c))||1},$=!0;function L(P){let Y=P[0].intersectionRatio;if(Y!==c){if(!$)return n();Y?n(!1,Y):r=setTimeout(()=>{n(!1,1e-7)},1e3)}$=!1}try{o=new IntersectionObserver(L,B(S({},w),{root:i.ownerDocument}))}catch{o=new IntersectionObserver(L,w)}o.observe(t)}return n(!0),s}function Ji(t,e,o,r){r===void 0&&(r={});let{ancestorScroll:i=!0,ancestorResize:s=!0,elementResize:n=typeof ResizeObserver=="function",layoutShift:l=typeof IntersectionObserver=="function",animationFrame:c=!1}=r,d=io(t),p=i||s?[...d?te(d):[],...te(e)]:[];p.forEach(v=>{i&&v.addEventListener("scroll",o,{passive:!0}),s&&v.addEventListener("resize",o)});let h=d&&l?Zi(d,o):null,m=-1,f=null;n&&(f=new ResizeObserver(v=>{let[w]=v;w&&w.target===d&&f&&(f.unobserve(e),cancelAnimationFrame(m),m=requestAnimationFrame(()=>{var $;($=f)==null||$.observe(e)})),o()}),d&&!c&&f.observe(d),f.observe(e));let g,b=c?Et(t):null;c&&_();function _(){let v=Et(t);b&&(v.x!==b.x||v.y!==b.y||v.width!==b.width||v.height!==b.height)&&o(),b=v,g=requestAnimationFrame(_)}return o(),()=>{var v;p.forEach(w=>{i&&w.removeEventListener("scroll",o),s&&w.removeEventListener("resize",o)}),h?.(),(v=f)==null||v.disconnect(),f=null,c&&cancelAnimationFrame(g)}}var Qi=Pi,ts=Di,es=Ti,Or=Ri,os=zi,rs=(t,e,o)=>{let r=new Map,i=S({platform:_e},o),s=B(S({},i.platform),{_c:r});return Oi(t,e,B(S({},i),{platform:s}))};function is(t){return ss(t)}function Ge(t){return t.assignedSlot?t.assignedSlot:t.parentNode instanceof ShadowRoot?t.parentNode.host:t.parentNode}function ss(t){for(let e=t;e;e=Ge(e))if(e instanceof Element&&getComputedStyle(e).display==="none")return null;for(let e=Ge(t);e;e=Ge(e)){if(!(e instanceof Element))continue;let o=getComputedStyle(e);if(o.display!=="contents"&&(o.position!=="static"||Se(o)||e.tagName==="BODY"))return e}return null}function ns(t){return t!==null&&typeof t=="object"&&"getBoundingClientRect"in t&&("contextElement"in t?t.contextElement instanceof Element:!0)}var x=class extends E{constructor(){super(...arguments),this.localize=new F(this),this.active=!1,this.placement="top",this.strategy="absolute",this.distance=0,this.skidding=0,this.arrow=!1,this.arrowPlacement="anchor",this.arrowPadding=10,this.flip=!1,this.flipFallbackPlacements="",this.flipFallbackStrategy="best-fit",this.flipPadding=0,this.shift=!1,this.shiftPadding=0,this.autoSizePadding=0,this.hoverBridge=!1,this.updateHoverBridge=()=>{if(this.hoverBridge&&this.anchorEl){let t=this.anchorEl.getBoundingClientRect(),e=this.popup.getBoundingClientRect(),o=this.placement.includes("top")||this.placement.includes("bottom"),r=0,i=0,s=0,n=0,l=0,c=0,d=0,p=0;o?t.top<e.top?(r=t.left,i=t.bottom,s=t.right,n=t.bottom,l=e.left,c=e.top,d=e.right,p=e.top):(r=e.left,i=e.bottom,s=e.right,n=e.bottom,l=t.left,c=t.top,d=t.right,p=t.top):t.left<e.left?(r=t.right,i=t.top,s=e.left,n=e.top,l=t.right,c=t.bottom,d=e.left,p=e.bottom):(r=e.right,i=e.top,s=t.left,n=t.top,l=e.right,c=e.bottom,d=t.left,p=t.bottom),this.style.setProperty("--hover-bridge-top-left-x",`${r}px`),this.style.setProperty("--hover-bridge-top-left-y",`${i}px`),this.style.setProperty("--hover-bridge-top-right-x",`${s}px`),this.style.setProperty("--hover-bridge-top-right-y",`${n}px`),this.style.setProperty("--hover-bridge-bottom-left-x",`${l}px`),this.style.setProperty("--hover-bridge-bottom-left-y",`${c}px`),this.style.setProperty("--hover-bridge-bottom-right-x",`${d}px`),this.style.setProperty("--hover-bridge-bottom-right-y",`${p}px`)}}}async connectedCallback(){super.connectedCallback(),await this.updateComplete,this.start()}disconnectedCallback(){super.disconnectedCallback(),this.stop()}async updated(t){super.updated(t),t.has("active")&&(this.active?this.start():this.stop()),t.has("anchor")&&this.handleAnchorChange(),this.active&&(await this.updateComplete,this.reposition())}async handleAnchorChange(){if(await this.stop(),this.anchor&&typeof this.anchor=="string"){let t=this.getRootNode();this.anchorEl=t.getElementById(this.anchor)}else this.anchor instanceof Element||ns(this.anchor)?this.anchorEl=this.anchor:this.anchorEl=this.querySelector('[slot="anchor"]');this.anchorEl instanceof HTMLSlotElement&&(this.anchorEl=this.anchorEl.assignedElements({flatten:!0})[0]),this.anchorEl&&this.active&&this.start()}start(){!this.anchorEl||!this.active||(this.cleanup=Ji(this.anchorEl,this.popup,()=>{this.reposition()}))}async stop(){return new Promise(t=>{this.cleanup?(this.cleanup(),this.cleanup=void 0,this.removeAttribute("data-current-placement"),this.style.removeProperty("--auto-size-available-width"),this.style.removeProperty("--auto-size-available-height"),requestAnimationFrame(()=>t())):t()})}reposition(){if(!this.active||!this.anchorEl)return;let t=[Qi({mainAxis:this.distance,crossAxis:this.skidding})];this.sync?t.push(Or({apply:({rects:o})=>{let r=this.sync==="width"||this.sync==="both",i=this.sync==="height"||this.sync==="both";this.popup.style.width=r?`${o.reference.width}px`:"",this.popup.style.height=i?`${o.reference.height}px`:""}})):(this.popup.style.width="",this.popup.style.height=""),this.flip&&t.push(es({boundary:this.flipBoundary,fallbackPlacements:this.flipFallbackPlacements,fallbackStrategy:this.flipFallbackStrategy==="best-fit"?"bestFit":"initialPlacement",padding:this.flipPadding})),this.shift&&t.push(ts({boundary:this.shiftBoundary,padding:this.shiftPadding})),this.autoSize?t.push(Or({boundary:this.autoSizeBoundary,padding:this.autoSizePadding,apply:({availableWidth:o,availableHeight:r})=>{this.autoSize==="vertical"||this.autoSize==="both"?this.style.setProperty("--auto-size-available-height",`${r}px`):this.style.removeProperty("--auto-size-available-height"),this.autoSize==="horizontal"||this.autoSize==="both"?this.style.setProperty("--auto-size-available-width",`${o}px`):this.style.removeProperty("--auto-size-available-width")}})):(this.style.removeProperty("--auto-size-available-width"),this.style.removeProperty("--auto-size-available-height")),this.arrow&&t.push(os({element:this.arrowEl,padding:this.arrowPadding}));let e=this.strategy==="absolute"?o=>_e.getOffsetParent(o,is):_e.getOffsetParent;rs(this.anchorEl,this.popup,{placement:this.placement,middleware:t,strategy:this.strategy,platform:B(S({},_e),{getOffsetParent:e})}).then(({x:o,y:r,middlewareData:i,placement:s})=>{let n=this.localize.dir()==="rtl",l={top:"bottom",right:"left",bottom:"top",left:"right"}[s.split("-")[0]];if(this.setAttribute("data-current-placement",s),Object.assign(this.popup.style,{left:`${o}px`,top:`${r}px`}),this.arrow){let c=i.arrow.x,d=i.arrow.y,p="",h="",m="",f="";if(this.arrowPlacement==="start"){let g=typeof c=="number"?`calc(${this.arrowPadding}px - var(--arrow-padding-offset))`:"";p=typeof d=="number"?`calc(${this.arrowPadding}px - var(--arrow-padding-offset))`:"",h=n?g:"",f=n?"":g}else if(this.arrowPlacement==="end"){let g=typeof c=="number"?`calc(${this.arrowPadding}px - var(--arrow-padding-offset))`:"";h=n?"":g,f=n?g:"",m=typeof d=="number"?`calc(${this.arrowPadding}px - var(--arrow-padding-offset))`:""}else this.arrowPlacement==="center"?(f=typeof c=="number"?"calc(50% - var(--arrow-size-diagonal))":"",p=typeof d=="number"?"calc(50% - var(--arrow-size-diagonal))":""):(f=typeof c=="number"?`${c}px`:"",p=typeof d=="number"?`${d}px`:"");Object.assign(this.arrowEl.style,{top:p,right:h,bottom:m,left:f,[l]:"calc(var(--arrow-size-diagonal) * -1)"})}}),requestAnimationFrame(()=>this.updateHoverBridge()),this.emit("sl-reposition")}render(){return k`
      <slot name="anchor" @slotchange=${this.handleAnchorChange}></slot>

      <span
        part="hover-bridge"
        class=${V({"popup-hover-bridge":!0,"popup-hover-bridge--visible":this.hoverBridge&&this.active})}
      ></span>

      <div
        part="popup"
        class=${V({popup:!0,"popup--active":this.active,"popup--fixed":this.strategy==="fixed","popup--has-arrow":this.arrow})}
      >
        <slot></slot>
        ${this.arrow?k`<div part="arrow" class="popup__arrow" role="presentation"></div>`:""}
      </div>
    `}};x.styles=[D,$r];a([O(".popup")],x.prototype,"popup",2);a([O(".popup__arrow")],x.prototype,"arrowEl",2);a([u()],x.prototype,"anchor",2);a([u({type:Boolean,reflect:!0})],x.prototype,"active",2);a([u({reflect:!0})],x.prototype,"placement",2);a([u({reflect:!0})],x.prototype,"strategy",2);a([u({type:Number})],x.prototype,"distance",2);a([u({type:Number})],x.prototype,"skidding",2);a([u({type:Boolean})],x.prototype,"arrow",2);a([u({attribute:"arrow-placement"})],x.prototype,"arrowPlacement",2);a([u({attribute:"arrow-padding",type:Number})],x.prototype,"arrowPadding",2);a([u({type:Boolean})],x.prototype,"flip",2);a([u({attribute:"flip-fallback-placements",converter:{fromAttribute:t=>t.split(" ").map(e=>e.trim()).filter(e=>e!==""),toAttribute:t=>t.join(" ")}})],x.prototype,"flipFallbackPlacements",2);a([u({attribute:"flip-fallback-strategy"})],x.prototype,"flipFallbackStrategy",2);a([u({type:Object})],x.prototype,"flipBoundary",2);a([u({attribute:"flip-padding",type:Number})],x.prototype,"flipPadding",2);a([u({type:Boolean})],x.prototype,"shift",2);a([u({type:Object})],x.prototype,"shiftBoundary",2);a([u({attribute:"shift-padding",type:Number})],x.prototype,"shiftPadding",2);a([u({attribute:"auto-size"})],x.prototype,"autoSize",2);a([u()],x.prototype,"sync",2);a([u({type:Object})],x.prototype,"autoSizeBoundary",2);a([u({attribute:"auto-size-padding",type:Number})],x.prototype,"autoSizePadding",2);a([u({attribute:"hover-bridge",type:Boolean})],x.prototype,"hoverBridge",2);var Ir=new Map,ls=new WeakMap;function as(t){return t??{keyframes:[],options:{duration:0}}}function Nr(t,e){return e.toLowerCase()==="rtl"?{keyframes:t.rtlKeyframes||t.keyframes,options:t.options}:t}function ct(t,e){Ir.set(t,as(e))}function ut(t,e,o){let r=ls.get(t);if(r?.[e])return Nr(r[e],o.dir);let i=Ir.get(e);return i?Nr(i,o.dir):{keyframes:[],options:{duration:0}}}function dt(t,e){return new Promise(o=>{function r(i){i.target===t&&(t.removeEventListener(e,r),o())}t.addEventListener(e,r)})}function ht(t,e,o){return new Promise(r=>{if(o?.duration===1/0)throw new Error("Promise-based animations must be finite.");let i=t.animate(e,B(S({},o),{duration:cs()?0:o.duration}));i.addEventListener("cancel",r,{once:!0}),i.addEventListener("finish",r,{once:!0})})}function no(t){return t=t.toString().toLowerCase(),t.indexOf("ms")>-1?parseFloat(t):t.indexOf("s")>-1?parseFloat(t)*1e3:parseFloat(t)}function cs(){return window.matchMedia("(prefers-reduced-motion: reduce)").matches}function pt(t){return Promise.all(t.getAnimations().map(e=>new Promise(o=>{e.cancel(),requestAnimationFrame(o)})))}var I=class extends E{constructor(){super(...arguments),this.localize=new F(this),this.open=!1,this.placement="bottom-start",this.disabled=!1,this.stayOpenOnSelect=!1,this.distance=0,this.skidding=0,this.hoist=!1,this.sync=void 0,this.handleKeyDown=t=>{this.open&&t.key==="Escape"&&(t.stopPropagation(),this.hide(),this.focusOnTrigger())},this.handleDocumentKeyDown=t=>{var e;if(t.key==="Escape"&&this.open&&!this.closeWatcher){t.stopPropagation(),this.focusOnTrigger(),this.hide();return}if(t.key==="Tab"){if(this.open&&((e=document.activeElement)==null?void 0:e.tagName.toLowerCase())==="sl-menu-item"){t.preventDefault(),this.hide(),this.focusOnTrigger();return}let o=(r,i)=>{if(!r)return null;let s=r.closest(i);if(s)return s;let n=r.getRootNode();return n instanceof ShadowRoot?o(n.host,i):null};setTimeout(()=>{var r;let i=((r=this.containingElement)==null?void 0:r.getRootNode())instanceof ShadowRoot?wr():document.activeElement;(!this.containingElement||o(i,this.containingElement.tagName.toLowerCase())!==this.containingElement)&&this.hide()})}},this.handleDocumentMouseDown=t=>{let e=t.composedPath();this.containingElement&&!e.includes(this.containingElement)&&this.hide()},this.handlePanelSelect=t=>{let e=t.target;!this.stayOpenOnSelect&&e.tagName.toLowerCase()==="sl-menu"&&(this.hide(),this.focusOnTrigger())}}connectedCallback(){super.connectedCallback(),this.containingElement||(this.containingElement=this)}firstUpdated(){this.panel.hidden=!this.open,this.open&&(this.addOpenListeners(),this.popup.active=!0)}disconnectedCallback(){super.disconnectedCallback(),this.removeOpenListeners(),this.hide()}focusOnTrigger(){let t=this.trigger.assignedElements({flatten:!0})[0];typeof t?.focus=="function"&&t.focus()}getMenu(){return this.panel.assignedElements({flatten:!0}).find(t=>t.tagName.toLowerCase()==="sl-menu")}handleTriggerClick(){this.open?this.hide():(this.show(),this.focusOnTrigger())}async handleTriggerKeyDown(t){if([" ","Enter"].includes(t.key)){t.preventDefault(),this.handleTriggerClick();return}let e=this.getMenu();if(e){let o=e.getAllItems(),r=o[0],i=o[o.length-1];["ArrowDown","ArrowUp","Home","End"].includes(t.key)&&(t.preventDefault(),this.open||(this.show(),await this.updateComplete),o.length>0&&this.updateComplete.then(()=>{(t.key==="ArrowDown"||t.key==="Home")&&(e.setCurrentItem(r),r.focus()),(t.key==="ArrowUp"||t.key==="End")&&(e.setCurrentItem(i),i.focus())}))}}handleTriggerKeyUp(t){t.key===" "&&t.preventDefault()}handleTriggerSlotChange(){this.updateAccessibleTrigger()}updateAccessibleTrigger(){let e=this.trigger.assignedElements({flatten:!0}).find(r=>Cr(r).start),o;if(e){switch(e.tagName.toLowerCase()){case"sl-button":case"sl-icon-button":o=e.button;break;default:o=e}o.setAttribute("aria-haspopup","true"),o.setAttribute("aria-expanded",this.open?"true":"false")}}async show(){if(!this.open)return this.open=!0,dt(this,"sl-after-show")}async hide(){if(this.open)return this.open=!1,dt(this,"sl-after-hide")}reposition(){this.popup.reposition()}addOpenListeners(){var t;this.panel.addEventListener("sl-select",this.handlePanelSelect),"CloseWatcher"in window?((t=this.closeWatcher)==null||t.destroy(),this.closeWatcher=new CloseWatcher,this.closeWatcher.onclose=()=>{this.hide(),this.focusOnTrigger()}):this.panel.addEventListener("keydown",this.handleKeyDown),document.addEventListener("keydown",this.handleDocumentKeyDown),document.addEventListener("mousedown",this.handleDocumentMouseDown)}removeOpenListeners(){var t;this.panel&&(this.panel.removeEventListener("sl-select",this.handlePanelSelect),this.panel.removeEventListener("keydown",this.handleKeyDown)),document.removeEventListener("keydown",this.handleDocumentKeyDown),document.removeEventListener("mousedown",this.handleDocumentMouseDown),(t=this.closeWatcher)==null||t.destroy()}async handleOpenChange(){if(this.disabled){this.open=!1;return}if(this.updateAccessibleTrigger(),this.open){this.emit("sl-show"),this.addOpenListeners(),await pt(this),this.panel.hidden=!1,this.popup.active=!0;let{keyframes:t,options:e}=ut(this,"dropdown.show",{dir:this.localize.dir()});await ht(this.popup.popup,t,e),this.emit("sl-after-show")}else{this.emit("sl-hide"),this.removeOpenListeners(),await pt(this);let{keyframes:t,options:e}=ut(this,"dropdown.hide",{dir:this.localize.dir()});await ht(this.popup.popup,t,e),this.panel.hidden=!0,this.popup.active=!1,this.emit("sl-after-hide")}}render(){return k`
      <sl-popup
        part="base"
        exportparts="popup:base__popup"
        id="dropdown"
        placement=${this.placement}
        distance=${this.distance}
        skidding=${this.skidding}
        strategy=${this.hoist?"fixed":"absolute"}
        flip
        shift
        auto-size="vertical"
        auto-size-padding="10"
        sync=${T(this.sync?this.sync:void 0)}
        class=${V({dropdown:!0,"dropdown--open":this.open})}
      >
        <slot
          name="trigger"
          slot="anchor"
          part="trigger"
          class="dropdown__trigger"
          @click=${this.handleTriggerClick}
          @keydown=${this.handleTriggerKeyDown}
          @keyup=${this.handleTriggerKeyUp}
          @slotchange=${this.handleTriggerSlotChange}
        ></slot>

        <div aria-hidden=${this.open?"false":"true"} aria-labelledby="dropdown">
          <slot part="panel" class="dropdown__panel"></slot>
        </div>
      </sl-popup>
    `}};I.styles=[D,vr];I.dependencies={"sl-popup":x};a([O(".dropdown")],I.prototype,"popup",2);a([O(".dropdown__trigger")],I.prototype,"trigger",2);a([O(".dropdown__panel")],I.prototype,"panel",2);a([u({type:Boolean,reflect:!0})],I.prototype,"open",2);a([u({reflect:!0})],I.prototype,"placement",2);a([u({type:Boolean,reflect:!0})],I.prototype,"disabled",2);a([u({attribute:"stay-open-on-select",type:Boolean,reflect:!0})],I.prototype,"stayOpenOnSelect",2);a([u({attribute:!1})],I.prototype,"containingElement",2);a([u({type:Number})],I.prototype,"distance",2);a([u({type:Number})],I.prototype,"skidding",2);a([u({type:Boolean})],I.prototype,"hoist",2);a([u({reflect:!0})],I.prototype,"sync",2);a([M("open",{waitUntilFirstUpdate:!0})],I.prototype,"handleOpenChange",1);ct("dropdown.show",{keyframes:[{opacity:0,scale:.9},{opacity:1,scale:1}],options:{duration:100,easing:"ease"}});ct("dropdown.hide",{keyframes:[{opacity:1,scale:1},{opacity:0,scale:.9}],options:{duration:100,easing:"ease"}});I.define("sl-dropdown");var Fr=A`
  :host {
    display: block;
    user-select: none;
    -webkit-user-select: none;
  }

  :host(:focus) {
    outline: none;
  }

  .option {
    position: relative;
    display: flex;
    align-items: center;
    font-family: var(--sl-font-sans);
    font-size: var(--sl-font-size-medium);
    font-weight: var(--sl-font-weight-normal);
    line-height: var(--sl-line-height-normal);
    letter-spacing: var(--sl-letter-spacing-normal);
    color: var(--sl-color-neutral-700);
    padding: var(--sl-spacing-x-small) var(--sl-spacing-medium) var(--sl-spacing-x-small) var(--sl-spacing-x-small);
    transition: var(--sl-transition-fast) fill;
    cursor: pointer;
  }

  .option--hover:not(.option--current):not(.option--disabled) {
    background-color: var(--sl-color-neutral-100);
    color: var(--sl-color-neutral-1000);
  }

  .option--current,
  .option--current.option--disabled {
    background-color: var(--sl-color-primary-600);
    color: var(--sl-color-neutral-0);
    opacity: 1;
  }

  .option--disabled {
    outline: none;
    opacity: 0.5;
    cursor: not-allowed;
  }

  .option__label {
    flex: 1 1 auto;
    display: inline-block;
    line-height: var(--sl-line-height-dense);
  }

  .option .option__check {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    justify-content: center;
    visibility: hidden;
    padding-inline-end: var(--sl-spacing-2x-small);
  }

  .option--selected .option__check {
    visibility: visible;
  }

  .option__prefix,
  .option__suffix {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
  }

  .option__prefix::slotted(*) {
    margin-inline-end: var(--sl-spacing-x-small);
  }

  .option__suffix::slotted(*) {
    margin-inline-start: var(--sl-spacing-x-small);
  }

  @media (forced-colors: active) {
    :host(:hover:not([aria-disabled='true'])) .option {
      outline: dashed 1px SelectedItem;
      outline-offset: -1px;
    }
  }
`;var K=class extends E{constructor(){super(...arguments),this.localize=new F(this),this.isInitialized=!1,this.current=!1,this.selected=!1,this.hasHover=!1,this.value="",this.disabled=!1}connectedCallback(){super.connectedCallback(),this.setAttribute("role","option"),this.setAttribute("aria-selected","false")}handleDefaultSlotChange(){this.isInitialized?customElements.whenDefined("sl-select").then(()=>{let t=this.closest("sl-select");t&&t.handleDefaultSlotChange()}):this.isInitialized=!0}handleMouseEnter(){this.hasHover=!0}handleMouseLeave(){this.hasHover=!1}handleDisabledChange(){this.setAttribute("aria-disabled",this.disabled?"true":"false")}handleSelectedChange(){this.setAttribute("aria-selected",this.selected?"true":"false")}handleValueChange(){typeof this.value!="string"&&(this.value=String(this.value)),this.value.includes(" ")&&(console.error("Option values cannot include a space. All spaces have been replaced with underscores.",this),this.value=this.value.replace(/ /g,"_"))}getTextLabel(){let t=this.childNodes,e="";return[...t].forEach(o=>{o.nodeType===Node.ELEMENT_NODE&&(o.hasAttribute("slot")||(e+=o.textContent)),o.nodeType===Node.TEXT_NODE&&(e+=o.textContent)}),e.trim()}render(){return k`
      <div
        part="base"
        class=${V({option:!0,"option--current":this.current,"option--disabled":this.disabled,"option--selected":this.selected,"option--hover":this.hasHover})}
        @mouseenter=${this.handleMouseEnter}
        @mouseleave=${this.handleMouseLeave}
      >
        <sl-icon part="checked-icon" class="option__check" name="check" library="system" aria-hidden="true"></sl-icon>
        <slot part="prefix" name="prefix" class="option__prefix"></slot>
        <slot part="label" class="option__label" @slotchange=${this.handleDefaultSlotChange}></slot>
        <slot part="suffix" name="suffix" class="option__suffix"></slot>
      </div>
    `}};K.styles=[D,Fr];K.dependencies={"sl-icon":H};a([O(".option__label")],K.prototype,"defaultSlot",2);a([R()],K.prototype,"current",2);a([R()],K.prototype,"selected",2);a([R()],K.prototype,"hasHover",2);a([u({reflect:!0})],K.prototype,"value",2);a([u({type:Boolean,reflect:!0})],K.prototype,"disabled",2);a([M("disabled")],K.prototype,"handleDisabledChange",1);a([M("selected")],K.prototype,"handleSelectedChange",1);a([M("value")],K.prototype,"handleValueChange",1);K.define("sl-option");var Hr=A`
  :host {
    display: inline-block;
  }

  .tag {
    display: flex;
    align-items: center;
    border: solid 1px;
    line-height: 1;
    white-space: nowrap;
    user-select: none;
    -webkit-user-select: none;
  }

  .tag__remove::part(base) {
    color: inherit;
    padding: 0;
  }

  /*
   * Variant modifiers
   */

  .tag--primary {
    background-color: var(--sl-color-primary-50);
    border-color: var(--sl-color-primary-200);
    color: var(--sl-color-primary-800);
  }

  .tag--primary:active > sl-icon-button {
    color: var(--sl-color-primary-600);
  }

  .tag--success {
    background-color: var(--sl-color-success-50);
    border-color: var(--sl-color-success-200);
    color: var(--sl-color-success-800);
  }

  .tag--success:active > sl-icon-button {
    color: var(--sl-color-success-600);
  }

  .tag--neutral {
    background-color: var(--sl-color-neutral-50);
    border-color: var(--sl-color-neutral-200);
    color: var(--sl-color-neutral-800);
  }

  .tag--neutral:active > sl-icon-button {
    color: var(--sl-color-neutral-600);
  }

  .tag--warning {
    background-color: var(--sl-color-warning-50);
    border-color: var(--sl-color-warning-200);
    color: var(--sl-color-warning-800);
  }

  .tag--warning:active > sl-icon-button {
    color: var(--sl-color-warning-600);
  }

  .tag--danger {
    background-color: var(--sl-color-danger-50);
    border-color: var(--sl-color-danger-200);
    color: var(--sl-color-danger-800);
  }

  .tag--danger:active > sl-icon-button {
    color: var(--sl-color-danger-600);
  }

  /*
   * Size modifiers
   */

  .tag--small {
    font-size: var(--sl-button-font-size-small);
    height: calc(var(--sl-input-height-small) * 0.8);
    line-height: calc(var(--sl-input-height-small) - var(--sl-input-border-width) * 2);
    border-radius: var(--sl-input-border-radius-small);
    padding: 0 var(--sl-spacing-x-small);
  }

  .tag--medium {
    font-size: var(--sl-button-font-size-medium);
    height: calc(var(--sl-input-height-medium) * 0.8);
    line-height: calc(var(--sl-input-height-medium) - var(--sl-input-border-width) * 2);
    border-radius: var(--sl-input-border-radius-medium);
    padding: 0 var(--sl-spacing-small);
  }

  .tag--large {
    font-size: var(--sl-button-font-size-large);
    height: calc(var(--sl-input-height-large) * 0.8);
    line-height: calc(var(--sl-input-height-large) - var(--sl-input-border-width) * 2);
    border-radius: var(--sl-input-border-radius-large);
    padding: 0 var(--sl-spacing-medium);
  }

  .tag__remove {
    margin-inline-start: var(--sl-spacing-x-small);
  }

  /*
   * Pill modifier
   */

  .tag--pill {
    border-radius: var(--sl-border-radius-pill);
  }
`;var Ur=A`
  :host {
    display: inline-block;
    color: var(--sl-color-neutral-600);
  }

  .icon-button {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    background: none;
    border: none;
    border-radius: var(--sl-border-radius-medium);
    font-size: inherit;
    color: inherit;
    padding: var(--sl-spacing-x-small);
    cursor: pointer;
    transition: var(--sl-transition-x-fast) color;
    -webkit-appearance: none;
  }

  .icon-button:hover:not(.icon-button--disabled),
  .icon-button:focus-visible:not(.icon-button--disabled) {
    color: var(--sl-color-primary-600);
  }

  .icon-button:active:not(.icon-button--disabled) {
    color: var(--sl-color-primary-700);
  }

  .icon-button:focus {
    outline: none;
  }

  .icon-button--disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .icon-button:focus-visible {
    outline: var(--sl-focus-ring);
    outline-offset: var(--sl-focus-ring-offset);
  }

  .icon-button__icon {
    pointer-events: none;
  }
`;var U=class extends E{constructor(){super(...arguments),this.hasFocus=!1,this.label="",this.disabled=!1}handleBlur(){this.hasFocus=!1,this.emit("sl-blur")}handleFocus(){this.hasFocus=!0,this.emit("sl-focus")}handleClick(t){this.disabled&&(t.preventDefault(),t.stopPropagation())}click(){this.button.click()}focus(t){this.button.focus(t)}blur(){this.button.blur()}render(){let t=!!this.href,e=t?Rt`a`:Rt`button`;return Bt`
      <${e}
        part="base"
        class=${V({"icon-button":!0,"icon-button--disabled":!t&&this.disabled,"icon-button--focused":this.hasFocus})}
        ?disabled=${T(t?void 0:this.disabled)}
        type=${T(t?void 0:"button")}
        href=${T(t?this.href:void 0)}
        target=${T(t?this.target:void 0)}
        download=${T(t?this.download:void 0)}
        rel=${T(t&&this.target?"noreferrer noopener":void 0)}
        role=${T(t?void 0:"button")}
        aria-disabled=${this.disabled?"true":"false"}
        aria-label="${this.label}"
        tabindex=${this.disabled?"-1":"0"}
        @blur=${this.handleBlur}
        @focus=${this.handleFocus}
        @click=${this.handleClick}
      >
        <sl-icon
          class="icon-button__icon"
          name=${T(this.name)}
          library=${T(this.library)}
          src=${T(this.src)}
          aria-hidden="true"
        ></sl-icon>
      </${e}>
    `}};U.styles=[D,Ur];U.dependencies={"sl-icon":H};a([O(".icon-button")],U.prototype,"button",2);a([R()],U.prototype,"hasFocus",2);a([u()],U.prototype,"name",2);a([u()],U.prototype,"library",2);a([u()],U.prototype,"src",2);a([u()],U.prototype,"href",2);a([u()],U.prototype,"target",2);a([u()],U.prototype,"download",2);a([u()],U.prototype,"label",2);a([u({type:Boolean,reflect:!0})],U.prototype,"disabled",2);var _t=class extends E{constructor(){super(...arguments),this.localize=new F(this),this.variant="neutral",this.size="medium",this.pill=!1,this.removable=!1}handleRemoveClick(){this.emit("sl-remove")}render(){return k`
      <span
        part="base"
        class=${V({tag:!0,"tag--primary":this.variant==="primary","tag--success":this.variant==="success","tag--neutral":this.variant==="neutral","tag--warning":this.variant==="warning","tag--danger":this.variant==="danger","tag--text":this.variant==="text","tag--small":this.size==="small","tag--medium":this.size==="medium","tag--large":this.size==="large","tag--pill":this.pill,"tag--removable":this.removable})}
      >
        <slot part="content" class="tag__content"></slot>

        ${this.removable?k`
              <sl-icon-button
                part="remove-button"
                exportparts="base:remove-button__base"
                name="x-lg"
                library="system"
                label=${this.localize.term("remove")}
                class="tag__remove"
                @click=${this.handleRemoveClick}
                tabindex="-1"
              ></sl-icon-button>
            `:""}
      </span>
    `}};_t.styles=[D,Hr];_t.dependencies={"sl-icon-button":U};a([u({reflect:!0})],_t.prototype,"variant",2);a([u({reflect:!0})],_t.prototype,"size",2);a([u({type:Boolean,reflect:!0})],_t.prototype,"pill",2);a([u({type:Boolean})],_t.prototype,"removable",2);var Wr=A`
  :host {
    display: block;
  }

  /** The popup */
  .select {
    flex: 1 1 auto;
    display: inline-flex;
    width: 100%;
    position: relative;
    vertical-align: middle;
  }

  .select::part(popup) {
    z-index: var(--sl-z-index-dropdown);
  }

  .select[data-current-placement^='top']::part(popup) {
    transform-origin: bottom;
  }

  .select[data-current-placement^='bottom']::part(popup) {
    transform-origin: top;
  }

  /* Combobox */
  .select__combobox {
    flex: 1;
    display: flex;
    width: 100%;
    min-width: 0;
    position: relative;
    align-items: center;
    justify-content: start;
    font-family: var(--sl-input-font-family);
    font-weight: var(--sl-input-font-weight);
    letter-spacing: var(--sl-input-letter-spacing);
    vertical-align: middle;
    overflow: hidden;
    cursor: pointer;
    transition:
      var(--sl-transition-fast) color,
      var(--sl-transition-fast) border,
      var(--sl-transition-fast) box-shadow,
      var(--sl-transition-fast) background-color;
  }

  .select__display-input {
    position: relative;
    width: 100%;
    font: inherit;
    border: none;
    background: none;
    color: var(--sl-input-color);
    cursor: inherit;
    overflow: hidden;
    padding: 0;
    margin: 0;
    -webkit-appearance: none;
  }

  .select__display-input::placeholder {
    color: var(--sl-input-placeholder-color);
  }

  .select:not(.select--disabled):hover .select__display-input {
    color: var(--sl-input-color-hover);
  }

  .select__display-input:focus {
    outline: none;
  }

  /* Visually hide the display input when multiple is enabled */
  .select--multiple:not(.select--placeholder-visible) .select__display-input {
    position: absolute;
    z-index: -1;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    opacity: 0;
  }

  .select__value-input {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    padding: 0;
    margin: 0;
    opacity: 0;
    z-index: -1;
  }

  .select__tags {
    display: flex;
    flex: 1;
    align-items: center;
    flex-wrap: wrap;
    margin-inline-start: var(--sl-spacing-2x-small);
  }

  .select__tags::slotted(sl-tag) {
    cursor: pointer !important;
  }

  .select--disabled .select__tags,
  .select--disabled .select__tags::slotted(sl-tag) {
    cursor: not-allowed !important;
  }

  /* Standard selects */
  .select--standard .select__combobox {
    background-color: var(--sl-input-background-color);
    border: solid var(--sl-input-border-width) var(--sl-input-border-color);
  }

  .select--standard.select--disabled .select__combobox {
    background-color: var(--sl-input-background-color-disabled);
    border-color: var(--sl-input-border-color-disabled);
    color: var(--sl-input-color-disabled);
    opacity: 0.5;
    cursor: not-allowed;
    outline: none;
  }

  .select--standard:not(.select--disabled).select--open .select__combobox,
  .select--standard:not(.select--disabled).select--focused .select__combobox {
    background-color: var(--sl-input-background-color-focus);
    border-color: var(--sl-input-border-color-focus);
    box-shadow: 0 0 0 var(--sl-focus-ring-width) var(--sl-input-focus-ring-color);
  }

  /* Filled selects */
  .select--filled .select__combobox {
    border: none;
    background-color: var(--sl-input-filled-background-color);
    color: var(--sl-input-color);
  }

  .select--filled:hover:not(.select--disabled) .select__combobox {
    background-color: var(--sl-input-filled-background-color-hover);
  }

  .select--filled.select--disabled .select__combobox {
    background-color: var(--sl-input-filled-background-color-disabled);
    opacity: 0.5;
    cursor: not-allowed;
  }

  .select--filled:not(.select--disabled).select--open .select__combobox,
  .select--filled:not(.select--disabled).select--focused .select__combobox {
    background-color: var(--sl-input-filled-background-color-focus);
    outline: var(--sl-focus-ring);
  }

  /* Sizes */
  .select--small .select__combobox {
    border-radius: var(--sl-input-border-radius-small);
    font-size: var(--sl-input-font-size-small);
    min-height: var(--sl-input-height-small);
    padding-block: 0;
    padding-inline: var(--sl-input-spacing-small);
  }

  .select--small .select__clear {
    margin-inline-start: var(--sl-input-spacing-small);
  }

  .select--small .select__prefix::slotted(*) {
    margin-inline-end: var(--sl-input-spacing-small);
  }

  .select--small.select--multiple:not(.select--placeholder-visible) .select__prefix::slotted(*) {
    margin-inline-start: var(--sl-input-spacing-small);
  }

  .select--small.select--multiple:not(.select--placeholder-visible) .select__combobox {
    padding-block: 2px;
    padding-inline-start: 0;
  }

  .select--small .select__tags {
    gap: 2px;
  }

  .select--medium .select__combobox {
    border-radius: var(--sl-input-border-radius-medium);
    font-size: var(--sl-input-font-size-medium);
    min-height: var(--sl-input-height-medium);
    padding-block: 0;
    padding-inline: var(--sl-input-spacing-medium);
  }

  .select--medium .select__clear {
    margin-inline-start: var(--sl-input-spacing-medium);
  }

  .select--medium .select__prefix::slotted(*) {
    margin-inline-end: var(--sl-input-spacing-medium);
  }

  .select--medium.select--multiple:not(.select--placeholder-visible) .select__prefix::slotted(*) {
    margin-inline-start: var(--sl-input-spacing-medium);
  }

  .select--medium.select--multiple:not(.select--placeholder-visible) .select__combobox {
    padding-inline-start: 0;
    padding-block: 3px;
  }

  .select--medium .select__tags {
    gap: 3px;
  }

  .select--large .select__combobox {
    border-radius: var(--sl-input-border-radius-large);
    font-size: var(--sl-input-font-size-large);
    min-height: var(--sl-input-height-large);
    padding-block: 0;
    padding-inline: var(--sl-input-spacing-large);
  }

  .select--large .select__clear {
    margin-inline-start: var(--sl-input-spacing-large);
  }

  .select--large .select__prefix::slotted(*) {
    margin-inline-end: var(--sl-input-spacing-large);
  }

  .select--large.select--multiple:not(.select--placeholder-visible) .select__prefix::slotted(*) {
    margin-inline-start: var(--sl-input-spacing-large);
  }

  .select--large.select--multiple:not(.select--placeholder-visible) .select__combobox {
    padding-inline-start: 0;
    padding-block: 4px;
  }

  .select--large .select__tags {
    gap: 4px;
  }

  /* Pills */
  .select--pill.select--small .select__combobox {
    border-radius: var(--sl-input-height-small);
  }

  .select--pill.select--medium .select__combobox {
    border-radius: var(--sl-input-height-medium);
  }

  .select--pill.select--large .select__combobox {
    border-radius: var(--sl-input-height-large);
  }

  /* Prefix and Suffix */
  .select__prefix,
  .select__suffix {
    flex: 0;
    display: inline-flex;
    align-items: center;
    color: var(--sl-input-placeholder-color);
  }

  .select__suffix::slotted(*) {
    margin-inline-start: var(--sl-spacing-small);
  }

  /* Clear button */
  .select__clear {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: inherit;
    color: var(--sl-input-icon-color);
    border: none;
    background: none;
    padding: 0;
    transition: var(--sl-transition-fast) color;
    cursor: pointer;
  }

  .select__clear:hover {
    color: var(--sl-input-icon-color-hover);
  }

  .select__clear:focus {
    outline: none;
  }

  /* Expand icon */
  .select__expand-icon {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    transition: var(--sl-transition-medium) rotate ease;
    rotate: 0;
    margin-inline-start: var(--sl-spacing-small);
  }

  .select--open .select__expand-icon {
    rotate: -180deg;
  }

  /* Listbox */
  .select__listbox {
    display: block;
    position: relative;
    font-family: var(--sl-font-sans);
    font-size: var(--sl-font-size-medium);
    font-weight: var(--sl-font-weight-normal);
    box-shadow: var(--sl-shadow-large);
    background: var(--sl-panel-background-color);
    border: solid var(--sl-panel-border-width) var(--sl-panel-border-color);
    border-radius: var(--sl-border-radius-medium);
    padding-block: var(--sl-spacing-x-small);
    padding-inline: 0;
    overflow: auto;
    overscroll-behavior: none;

    /* Make sure it adheres to the popup's auto size */
    max-width: var(--auto-size-available-width);
    max-height: var(--auto-size-available-height);
  }

  .select__listbox ::slotted(sl-divider) {
    --spacing: var(--sl-spacing-x-small);
  }

  .select__listbox ::slotted(small) {
    display: block;
    font-size: var(--sl-font-size-small);
    font-weight: var(--sl-font-weight-semibold);
    color: var(--sl-color-neutral-500);
    padding-block: var(--sl-spacing-2x-small);
    padding-inline: var(--sl-spacing-x-large);
  }
`;var lo=class extends ve{constructor(t){if(super(t),this.it=z,t.type!==ge.CHILD)throw Error(this.constructor.directiveName+"() can only be used in child bindings")}render(t){if(t===z||t==null)return this._t=void 0,this.it=t;if(t===rt)return t;if(typeof t!="string")throw Error(this.constructor.directiveName+"() called with a non-string value");if(t===this.it)return this._t;this.it=t;let e=[t];return e.raw=e,this._t={_$litType$:this.constructor.resultType,strings:e,values:[]}}};lo.directiveName="unsafeHTML",lo.resultType=1;var jr=be(lo);function us(t,e){return{top:Math.round(t.getBoundingClientRect().top-e.getBoundingClientRect().top),left:Math.round(t.getBoundingClientRect().left-e.getBoundingClientRect().left)}}function qr(t,e,o="vertical",r="smooth"){let i=us(t,e),s=i.top+e.scrollTop,n=i.left+e.scrollLeft,l=e.scrollLeft,c=e.scrollLeft+e.offsetWidth,d=e.scrollTop,p=e.scrollTop+e.offsetHeight;(o==="horizontal"||o==="both")&&(n<l?e.scrollTo({left:n,behavior:r}):n+t.clientWidth>c&&e.scrollTo({left:n-e.offsetWidth+t.clientWidth,behavior:r})),(o==="vertical"||o==="both")&&(s<d?e.scrollTo({top:s,behavior:r}):s+t.clientHeight>p&&e.scrollTo({top:s-e.offsetHeight+t.clientHeight,behavior:r}))}var Kr=A`
  .form-control .form-control__label {
    display: none;
  }

  .form-control .form-control__help-text {
    display: none;
  }

  /* Label */
  .form-control--has-label .form-control__label {
    display: inline-block;
    color: var(--sl-input-label-color);
    margin-bottom: var(--sl-spacing-3x-small);
  }

  .form-control--has-label.form-control--small .form-control__label {
    font-size: var(--sl-input-label-font-size-small);
  }

  .form-control--has-label.form-control--medium .form-control__label {
    font-size: var(--sl-input-label-font-size-medium);
  }

  .form-control--has-label.form-control--large .form-control__label {
    font-size: var(--sl-input-label-font-size-large);
  }

  :host([required]) .form-control--has-label .form-control__label::after {
    content: var(--sl-input-required-content);
    margin-inline-start: var(--sl-input-required-content-offset);
    color: var(--sl-input-required-content-color);
  }

  /* Help text */
  .form-control--has-help-text .form-control__help-text {
    display: block;
    color: var(--sl-input-help-text-color);
    margin-top: var(--sl-spacing-3x-small);
  }

  .form-control--has-help-text.form-control--small .form-control__help-text {
    font-size: var(--sl-input-help-text-font-size-small);
  }

  .form-control--has-help-text.form-control--medium .form-control__help-text {
    font-size: var(--sl-input-help-text-font-size-medium);
  }

  .form-control--has-help-text.form-control--large .form-control__help-text {
    font-size: var(--sl-input-help-text-font-size-large);
  }

  .form-control--has-help-text.form-control--radio-group .form-control__help-text {
    margin-top: var(--sl-spacing-2x-small);
  }
`;var y=class extends E{constructor(){super(...arguments),this.formControlController=new he(this,{assumeInteractionOn:["sl-blur","sl-input"]}),this.hasSlotController=new fe(this,"help-text","label"),this.localize=new F(this),this.typeToSelectString="",this.hasFocus=!1,this.displayLabel="",this.selectedOptions=[],this.valueHasChanged=!1,this.name="",this._value="",this.defaultValue="",this.size="medium",this.placeholder="",this.multiple=!1,this.maxOptionsVisible=3,this.disabled=!1,this.clearable=!1,this.open=!1,this.hoist=!1,this.filled=!1,this.pill=!1,this.label="",this.placement="bottom",this.helpText="",this.form="",this.required=!1,this.getTag=t=>k`
      <sl-tag
        part="tag"
        exportparts="
              base:tag__base,
              content:tag__content,
              remove-button:tag__remove-button,
              remove-button__base:tag__remove-button__base
            "
        ?pill=${this.pill}
        size=${this.size}
        removable
        @sl-remove=${e=>this.handleTagRemove(e,t)}
      >
        ${t.getTextLabel()}
      </sl-tag>
    `,this.handleDocumentFocusIn=t=>{let e=t.composedPath();this&&!e.includes(this)&&this.hide()},this.handleDocumentKeyDown=t=>{let e=t.target,o=e.closest(".select__clear")!==null,r=e.closest("sl-icon-button")!==null;if(!(o||r)){if(t.key==="Escape"&&this.open&&!this.closeWatcher&&(t.preventDefault(),t.stopPropagation(),this.hide(),this.displayInput.focus({preventScroll:!0})),t.key==="Enter"||t.key===" "&&this.typeToSelectString===""){if(t.preventDefault(),t.stopImmediatePropagation(),!this.open){this.show();return}this.currentOption&&!this.currentOption.disabled&&(this.valueHasChanged=!0,this.multiple?this.toggleOptionSelection(this.currentOption):this.setSelectedOptions(this.currentOption),this.updateComplete.then(()=>{this.emit("sl-input"),this.emit("sl-change")}),this.multiple||(this.hide(),this.displayInput.focus({preventScroll:!0})));return}if(["ArrowUp","ArrowDown","Home","End"].includes(t.key)){let i=this.getAllOptions(),s=i.indexOf(this.currentOption),n=Math.max(0,s);if(t.preventDefault(),!this.open&&(this.show(),this.currentOption))return;t.key==="ArrowDown"?(n=s+1,n>i.length-1&&(n=0)):t.key==="ArrowUp"?(n=s-1,n<0&&(n=i.length-1)):t.key==="Home"?n=0:t.key==="End"&&(n=i.length-1),this.setCurrentOption(i[n])}if(t.key&&t.key.length===1||t.key==="Backspace"){let i=this.getAllOptions();if(t.metaKey||t.ctrlKey||t.altKey)return;if(!this.open){if(t.key==="Backspace")return;this.show()}t.stopPropagation(),t.preventDefault(),clearTimeout(this.typeToSelectTimeout),this.typeToSelectTimeout=window.setTimeout(()=>this.typeToSelectString="",1e3),t.key==="Backspace"?this.typeToSelectString=this.typeToSelectString.slice(0,-1):this.typeToSelectString+=t.key.toLowerCase();for(let s of i)if(s.getTextLabel().toLowerCase().startsWith(this.typeToSelectString)){this.setCurrentOption(s);break}}}},this.handleDocumentMouseDown=t=>{let e=t.composedPath();this&&!e.includes(this)&&this.hide()}}get value(){return this._value}set value(t){this.multiple?t=Array.isArray(t)?t:t.split(" "):t=Array.isArray(t)?t.join(" "):t,this._value!==t&&(this.valueHasChanged=!0,this._value=t)}get validity(){return this.valueInput.validity}get validationMessage(){return this.valueInput.validationMessage}connectedCallback(){super.connectedCallback(),setTimeout(()=>{this.handleDefaultSlotChange()}),this.open=!1}addOpenListeners(){var t;document.addEventListener("focusin",this.handleDocumentFocusIn),document.addEventListener("keydown",this.handleDocumentKeyDown),document.addEventListener("mousedown",this.handleDocumentMouseDown),this.getRootNode()!==document&&this.getRootNode().addEventListener("focusin",this.handleDocumentFocusIn),"CloseWatcher"in window&&((t=this.closeWatcher)==null||t.destroy(),this.closeWatcher=new CloseWatcher,this.closeWatcher.onclose=()=>{this.open&&(this.hide(),this.displayInput.focus({preventScroll:!0}))})}removeOpenListeners(){var t;document.removeEventListener("focusin",this.handleDocumentFocusIn),document.removeEventListener("keydown",this.handleDocumentKeyDown),document.removeEventListener("mousedown",this.handleDocumentMouseDown),this.getRootNode()!==document&&this.getRootNode().removeEventListener("focusin",this.handleDocumentFocusIn),(t=this.closeWatcher)==null||t.destroy()}handleFocus(){this.hasFocus=!0,this.displayInput.setSelectionRange(0,0),this.emit("sl-focus")}handleBlur(){this.hasFocus=!1,this.emit("sl-blur")}handleLabelClick(){this.displayInput.focus()}handleComboboxMouseDown(t){let o=t.composedPath().some(r=>r instanceof Element&&r.tagName.toLowerCase()==="sl-icon-button");this.disabled||o||(t.preventDefault(),this.displayInput.focus({preventScroll:!0}),this.open=!this.open)}handleComboboxKeyDown(t){t.key!=="Tab"&&(t.stopPropagation(),this.handleDocumentKeyDown(t))}handleClearClick(t){t.stopPropagation(),this.valueHasChanged=!0,this.value!==""&&(this.setSelectedOptions([]),this.displayInput.focus({preventScroll:!0}),this.updateComplete.then(()=>{this.emit("sl-clear"),this.emit("sl-input"),this.emit("sl-change")}))}handleClearMouseDown(t){t.stopPropagation(),t.preventDefault()}handleOptionClick(t){let o=t.target.closest("sl-option"),r=this.value;o&&!o.disabled&&(this.valueHasChanged=!0,this.multiple?this.toggleOptionSelection(o):this.setSelectedOptions(o),this.updateComplete.then(()=>this.displayInput.focus({preventScroll:!0})),this.value!==r&&this.updateComplete.then(()=>{this.emit("sl-input"),this.emit("sl-change")}),this.multiple||(this.hide(),this.displayInput.focus({preventScroll:!0})))}handleDefaultSlotChange(){customElements.get("sl-option")||customElements.whenDefined("sl-option").then(()=>this.handleDefaultSlotChange());let t=this.getAllOptions(),e=this.valueHasChanged?this.value:this.defaultValue,o=Array.isArray(e)?e:[e],r=[];t.forEach(i=>r.push(i.value)),this.setSelectedOptions(t.filter(i=>o.includes(i.value)))}handleTagRemove(t,e){t.stopPropagation(),this.valueHasChanged=!0,this.disabled||(this.toggleOptionSelection(e,!1),this.updateComplete.then(()=>{this.emit("sl-input"),this.emit("sl-change")}))}getAllOptions(){return[...this.querySelectorAll("sl-option")]}getFirstOption(){return this.querySelector("sl-option")}setCurrentOption(t){this.getAllOptions().forEach(o=>{o.current=!1,o.tabIndex=-1}),t&&(this.currentOption=t,t.current=!0,t.tabIndex=0,t.focus())}setSelectedOptions(t){let e=this.getAllOptions(),o=Array.isArray(t)?t:[t];e.forEach(r=>r.selected=!1),o.length&&o.forEach(r=>r.selected=!0),this.selectionChanged()}toggleOptionSelection(t,e){e===!0||e===!1?t.selected=e:t.selected=!t.selected,this.selectionChanged()}selectionChanged(){var t,e,o;let r=this.getAllOptions();this.selectedOptions=r.filter(s=>s.selected);let i=this.valueHasChanged;if(this.multiple)this.value=this.selectedOptions.map(s=>s.value),this.placeholder&&this.value.length===0?this.displayLabel="":this.displayLabel=this.localize.term("numOptionsSelected",this.selectedOptions.length);else{let s=this.selectedOptions[0];this.value=(t=s?.value)!=null?t:"",this.displayLabel=(o=(e=s?.getTextLabel)==null?void 0:e.call(s))!=null?o:""}this.valueHasChanged=i,this.updateComplete.then(()=>{this.formControlController.updateValidity()})}get tags(){return this.selectedOptions.map((t,e)=>{if(e<this.maxOptionsVisible||this.maxOptionsVisible<=0){let o=this.getTag(t,e);return k`<div @sl-remove=${r=>this.handleTagRemove(r,t)}>
          ${typeof o=="string"?jr(o):o}
        </div>`}else if(e===this.maxOptionsVisible)return k`<sl-tag size=${this.size}>+${this.selectedOptions.length-e}</sl-tag>`;return k``})}handleInvalid(t){this.formControlController.setValidity(!1),this.formControlController.emitInvalidEvent(t)}handleDisabledChange(){this.disabled&&(this.open=!1,this.handleOpenChange())}attributeChangedCallback(t,e,o){if(super.attributeChangedCallback(t,e,o),t==="value"){let r=this.valueHasChanged;this.value=this.defaultValue,this.valueHasChanged=r}}handleValueChange(){if(!this.valueHasChanged){let o=this.valueHasChanged;this.value=this.defaultValue,this.valueHasChanged=o}let t=this.getAllOptions(),e=Array.isArray(this.value)?this.value:[this.value];this.setSelectedOptions(t.filter(o=>e.includes(o.value)))}async handleOpenChange(){if(this.open&&!this.disabled){this.setCurrentOption(this.selectedOptions[0]||this.getFirstOption()),this.emit("sl-show"),this.addOpenListeners(),await pt(this),this.listbox.hidden=!1,this.popup.active=!0,requestAnimationFrame(()=>{this.setCurrentOption(this.currentOption)});let{keyframes:t,options:e}=ut(this,"select.show",{dir:this.localize.dir()});await ht(this.popup.popup,t,e),this.currentOption&&qr(this.currentOption,this.listbox,"vertical","auto"),this.emit("sl-after-show")}else{this.emit("sl-hide"),this.removeOpenListeners(),await pt(this);let{keyframes:t,options:e}=ut(this,"select.hide",{dir:this.localize.dir()});await ht(this.popup.popup,t,e),this.listbox.hidden=!0,this.popup.active=!1,this.emit("sl-after-hide")}}async show(){if(this.open||this.disabled){this.open=!1;return}return this.open=!0,dt(this,"sl-after-show")}async hide(){if(!this.open||this.disabled){this.open=!1;return}return this.open=!1,dt(this,"sl-after-hide")}checkValidity(){return this.valueInput.checkValidity()}getForm(){return this.formControlController.getForm()}reportValidity(){return this.valueInput.reportValidity()}setCustomValidity(t){this.valueInput.setCustomValidity(t),this.formControlController.updateValidity()}focus(t){this.displayInput.focus(t)}blur(){this.displayInput.blur()}render(){let t=this.hasSlotController.test("label"),e=this.hasSlotController.test("help-text"),o=this.label?!0:!!t,r=this.helpText?!0:!!e,i=this.clearable&&!this.disabled&&this.value.length>0,s=this.placeholder&&this.value&&this.value.length<=0;return k`
      <div
        part="form-control"
        class=${V({"form-control":!0,"form-control--small":this.size==="small","form-control--medium":this.size==="medium","form-control--large":this.size==="large","form-control--has-label":o,"form-control--has-help-text":r})}
      >
        <label
          id="label"
          part="form-control-label"
          class="form-control__label"
          aria-hidden=${o?"false":"true"}
          @click=${this.handleLabelClick}
        >
          <slot name="label">${this.label}</slot>
        </label>

        <div part="form-control-input" class="form-control-input">
          <sl-popup
            class=${V({select:!0,"select--standard":!0,"select--filled":this.filled,"select--pill":this.pill,"select--open":this.open,"select--disabled":this.disabled,"select--multiple":this.multiple,"select--focused":this.hasFocus,"select--placeholder-visible":s,"select--top":this.placement==="top","select--bottom":this.placement==="bottom","select--small":this.size==="small","select--medium":this.size==="medium","select--large":this.size==="large"})}
            placement=${this.placement}
            strategy=${this.hoist?"fixed":"absolute"}
            flip
            shift
            sync="width"
            auto-size="vertical"
            auto-size-padding="10"
          >
            <div
              part="combobox"
              class="select__combobox"
              slot="anchor"
              @keydown=${this.handleComboboxKeyDown}
              @mousedown=${this.handleComboboxMouseDown}
            >
              <slot part="prefix" name="prefix" class="select__prefix"></slot>

              <input
                part="display-input"
                class="select__display-input"
                type="text"
                placeholder=${this.placeholder}
                .disabled=${this.disabled}
                .value=${this.displayLabel}
                autocomplete="off"
                spellcheck="false"
                autocapitalize="off"
                readonly
                aria-controls="listbox"
                aria-expanded=${this.open?"true":"false"}
                aria-haspopup="listbox"
                aria-labelledby="label"
                aria-disabled=${this.disabled?"true":"false"}
                aria-describedby="help-text"
                role="combobox"
                tabindex="0"
                @focus=${this.handleFocus}
                @blur=${this.handleBlur}
              />

              ${this.multiple?k`<div part="tags" class="select__tags">${this.tags}</div>`:""}

              <input
                class="select__value-input"
                type="text"
                ?disabled=${this.disabled}
                ?required=${this.required}
                .value=${Array.isArray(this.value)?this.value.join(", "):this.value}
                tabindex="-1"
                aria-hidden="true"
                @focus=${()=>this.focus()}
                @invalid=${this.handleInvalid}
              />

              ${i?k`
                    <button
                      part="clear-button"
                      class="select__clear"
                      type="button"
                      aria-label=${this.localize.term("clearEntry")}
                      @mousedown=${this.handleClearMouseDown}
                      @click=${this.handleClearClick}
                      tabindex="-1"
                    >
                      <slot name="clear-icon">
                        <sl-icon name="x-circle-fill" library="system"></sl-icon>
                      </slot>
                    </button>
                  `:""}

              <slot name="suffix" part="suffix" class="select__suffix"></slot>

              <slot name="expand-icon" part="expand-icon" class="select__expand-icon">
                <sl-icon library="system" name="chevron-down"></sl-icon>
              </slot>
            </div>

            <div
              id="listbox"
              role="listbox"
              aria-expanded=${this.open?"true":"false"}
              aria-multiselectable=${this.multiple?"true":"false"}
              aria-labelledby="label"
              part="listbox"
              class="select__listbox"
              tabindex="-1"
              @mouseup=${this.handleOptionClick}
              @slotchange=${this.handleDefaultSlotChange}
            >
              <slot></slot>
            </div>
          </sl-popup>
        </div>

        <div
          part="form-control-help-text"
          id="help-text"
          class="form-control__help-text"
          aria-hidden=${r?"false":"true"}
        >
          <slot name="help-text">${this.helpText}</slot>
        </div>
      </div>
    `}};y.styles=[D,Kr,Wr];y.dependencies={"sl-icon":H,"sl-popup":x,"sl-tag":_t};a([O(".select")],y.prototype,"popup",2);a([O(".select__combobox")],y.prototype,"combobox",2);a([O(".select__display-input")],y.prototype,"displayInput",2);a([O(".select__value-input")],y.prototype,"valueInput",2);a([O(".select__listbox")],y.prototype,"listbox",2);a([R()],y.prototype,"hasFocus",2);a([R()],y.prototype,"displayLabel",2);a([R()],y.prototype,"currentOption",2);a([R()],y.prototype,"selectedOptions",2);a([R()],y.prototype,"valueHasChanged",2);a([u()],y.prototype,"name",2);a([R()],y.prototype,"value",1);a([u({attribute:"value"})],y.prototype,"defaultValue",2);a([u({reflect:!0})],y.prototype,"size",2);a([u()],y.prototype,"placeholder",2);a([u({type:Boolean,reflect:!0})],y.prototype,"multiple",2);a([u({attribute:"max-options-visible",type:Number})],y.prototype,"maxOptionsVisible",2);a([u({type:Boolean,reflect:!0})],y.prototype,"disabled",2);a([u({type:Boolean})],y.prototype,"clearable",2);a([u({type:Boolean,reflect:!0})],y.prototype,"open",2);a([u({type:Boolean})],y.prototype,"hoist",2);a([u({type:Boolean,reflect:!0})],y.prototype,"filled",2);a([u({type:Boolean,reflect:!0})],y.prototype,"pill",2);a([u()],y.prototype,"label",2);a([u({reflect:!0})],y.prototype,"placement",2);a([u({attribute:"help-text"})],y.prototype,"helpText",2);a([u({reflect:!0})],y.prototype,"form",2);a([u({type:Boolean,reflect:!0})],y.prototype,"required",2);a([u()],y.prototype,"getTag",2);a([M("disabled",{waitUntilFirstUpdate:!0})],y.prototype,"handleDisabledChange",1);a([M(["defaultValue","value"],{waitUntilFirstUpdate:!0})],y.prototype,"handleValueChange",1);a([M("open",{waitUntilFirstUpdate:!0})],y.prototype,"handleOpenChange",1);ct("select.show",{keyframes:[{opacity:0,scale:.9},{opacity:1,scale:1}],options:{duration:100,easing:"ease"}});ct("select.hide",{keyframes:[{opacity:1,scale:1},{opacity:0,scale:.9}],options:{duration:100,easing:"ease"}});y.define("sl-select");var Yr=A`
  :host {
    --max-width: 20rem;
    --hide-delay: 0ms;
    --show-delay: 150ms;

    display: contents;
  }

  .tooltip {
    --arrow-size: var(--sl-tooltip-arrow-size);
    --arrow-color: var(--sl-tooltip-background-color);
  }

  .tooltip::part(popup) {
    z-index: var(--sl-z-index-tooltip);
  }

  .tooltip[placement^='top']::part(popup) {
    transform-origin: bottom;
  }

  .tooltip[placement^='bottom']::part(popup) {
    transform-origin: top;
  }

  .tooltip[placement^='left']::part(popup) {
    transform-origin: right;
  }

  .tooltip[placement^='right']::part(popup) {
    transform-origin: left;
  }

  .tooltip__body {
    display: block;
    width: max-content;
    max-width: var(--max-width);
    border-radius: var(--sl-tooltip-border-radius);
    background-color: var(--sl-tooltip-background-color);
    font-family: var(--sl-tooltip-font-family);
    font-size: var(--sl-tooltip-font-size);
    font-weight: var(--sl-tooltip-font-weight);
    line-height: var(--sl-tooltip-line-height);
    text-align: start;
    white-space: normal;
    color: var(--sl-tooltip-color);
    padding: var(--sl-tooltip-padding);
    pointer-events: none;
    user-select: none;
    -webkit-user-select: none;
  }
`;var N=class extends E{constructor(){super(),this.localize=new F(this),this.content="",this.placement="top",this.disabled=!1,this.distance=8,this.open=!1,this.skidding=0,this.trigger="hover focus",this.hoist=!1,this.handleBlur=()=>{this.hasTrigger("focus")&&this.hide()},this.handleClick=()=>{this.hasTrigger("click")&&(this.open?this.hide():this.show())},this.handleFocus=()=>{this.hasTrigger("focus")&&this.show()},this.handleDocumentKeyDown=t=>{t.key==="Escape"&&(t.stopPropagation(),this.hide())},this.handleMouseOver=()=>{if(this.hasTrigger("hover")){let t=no(getComputedStyle(this).getPropertyValue("--show-delay"));clearTimeout(this.hoverTimeout),this.hoverTimeout=window.setTimeout(()=>this.show(),t)}},this.handleMouseOut=()=>{if(this.hasTrigger("hover")){let t=no(getComputedStyle(this).getPropertyValue("--hide-delay"));clearTimeout(this.hoverTimeout),this.hoverTimeout=window.setTimeout(()=>this.hide(),t)}},this.addEventListener("blur",this.handleBlur,!0),this.addEventListener("focus",this.handleFocus,!0),this.addEventListener("click",this.handleClick),this.addEventListener("mouseover",this.handleMouseOver),this.addEventListener("mouseout",this.handleMouseOut)}disconnectedCallback(){var t;super.disconnectedCallback(),(t=this.closeWatcher)==null||t.destroy(),document.removeEventListener("keydown",this.handleDocumentKeyDown)}firstUpdated(){this.body.hidden=!this.open,this.open&&(this.popup.active=!0,this.popup.reposition())}hasTrigger(t){return this.trigger.split(" ").includes(t)}async handleOpenChange(){var t,e;if(this.open){if(this.disabled)return;this.emit("sl-show"),"CloseWatcher"in window?((t=this.closeWatcher)==null||t.destroy(),this.closeWatcher=new CloseWatcher,this.closeWatcher.onclose=()=>{this.hide()}):document.addEventListener("keydown",this.handleDocumentKeyDown),await pt(this.body),this.body.hidden=!1,this.popup.active=!0;let{keyframes:o,options:r}=ut(this,"tooltip.show",{dir:this.localize.dir()});await ht(this.popup.popup,o,r),this.popup.reposition(),this.emit("sl-after-show")}else{this.emit("sl-hide"),(e=this.closeWatcher)==null||e.destroy(),document.removeEventListener("keydown",this.handleDocumentKeyDown),await pt(this.body);let{keyframes:o,options:r}=ut(this,"tooltip.hide",{dir:this.localize.dir()});await ht(this.popup.popup,o,r),this.popup.active=!1,this.body.hidden=!0,this.emit("sl-after-hide")}}async handleOptionsChange(){this.hasUpdated&&(await this.updateComplete,this.popup.reposition())}handleDisabledChange(){this.disabled&&this.open&&this.hide()}async show(){if(!this.open)return this.open=!0,dt(this,"sl-after-show")}async hide(){if(this.open)return this.open=!1,dt(this,"sl-after-hide")}render(){return k`
      <sl-popup
        part="base"
        exportparts="
          popup:base__popup,
          arrow:base__arrow
        "
        class=${V({tooltip:!0,"tooltip--open":this.open})}
        placement=${this.placement}
        distance=${this.distance}
        skidding=${this.skidding}
        strategy=${this.hoist?"fixed":"absolute"}
        flip
        shift
        arrow
        hover-bridge
      >
        ${""}
        <slot slot="anchor" aria-describedby="tooltip"></slot>

        ${""}
        <div part="body" id="tooltip" class="tooltip__body" role="tooltip" aria-live=${this.open?"polite":"off"}>
          <slot name="content">${this.content}</slot>
        </div>
      </sl-popup>
    `}};N.styles=[D,Yr];N.dependencies={"sl-popup":x};a([O("slot:not([name])")],N.prototype,"defaultSlot",2);a([O(".tooltip__body")],N.prototype,"body",2);a([O("sl-popup")],N.prototype,"popup",2);a([u()],N.prototype,"content",2);a([u()],N.prototype,"placement",2);a([u({type:Boolean,reflect:!0})],N.prototype,"disabled",2);a([u({type:Number})],N.prototype,"distance",2);a([u({type:Boolean,reflect:!0})],N.prototype,"open",2);a([u({type:Number})],N.prototype,"skidding",2);a([u()],N.prototype,"trigger",2);a([u({type:Boolean})],N.prototype,"hoist",2);a([M("open",{waitUntilFirstUpdate:!0})],N.prototype,"handleOpenChange",1);a([M(["content","distance","hoist","placement","skidding"])],N.prototype,"handleOptionsChange",1);a([M("disabled")],N.prototype,"handleDisabledChange",1);ct("tooltip.show",{keyframes:[{opacity:0,scale:.8},{opacity:1,scale:1}],options:{duration:150,easing:"ease"}});ct("tooltip.hide",{keyframes:[{opacity:1,scale:1},{opacity:0,scale:.8}],options:{duration:150,easing:"ease"}});N.define("sl-tooltip");
